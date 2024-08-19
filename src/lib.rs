#![allow(non_snake_case, unused_variables)]
#![allow(dead_code)]

mod bitreader;
mod bitwriter;
mod clc;

use windows::Win32::Foundation::{BOOL, HANDLE};
use windows::Win32::Networking::WinSock::SOCKET;
use windows::Win32::Networking::WinSock::SOCKADDR;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{PCSTR, PCWSTR};

use std::os::raw::{c_void, c_char};
use std::error::Error;
use std::{ffi::CString, ffi::c_int, iter, mem};
use std::sync::{Mutex, LazyLock};

use retour::static_detour;
use bitreader::BitReader;
use clc::*;

static_detour! {
    static SendtoHook: unsafe extern "system" fn(SOCKET, *mut c_char, c_int, c_int, *mut SOCKADDR, c_int) -> c_int;
}

type FnSendto = unsafe extern "system" fn(SOCKET, *mut c_char, c_int, c_int, *mut SOCKADDR, c_int) -> c_int;

const PACKET_FLAG_RELIABLE:   u8 = 1 << 0;
const PACKET_FLAG_COMPRESSED: u8 = 1 << 1;
const PACKET_FLAG_ENCRYPTED:  u8 = 1 << 2;
const PACKET_FLAG_SPLIT:      u8 = 1 << 3;
const PACKET_FLAG_CHOKED:     u8 = 1 << 4;

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct PacketFlag(u8);

impl std::fmt::Debug for PacketFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = String::new();

        if self.0 & PACKET_FLAG_RELIABLE != 0 {
            flags.push_str("Reliable ");
        }
        if self.0 & PACKET_FLAG_COMPRESSED != 0 {
            flags.push_str("Compressed ");
        }
        if self.0 & PACKET_FLAG_ENCRYPTED != 0 {
            flags.push_str("Encrypted ");
        }
        if self.0 & PACKET_FLAG_SPLIT != 0 {
            flags.push_str("Split ");
        }
        if self.0 & PACKET_FLAG_CHOKED != 0 {
            flags.push_str("Chocked ");
        }

        write!(f, "{}({})", flags, self.0)
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct NetPacketHeader {
    sequence: u32,
    sequence_ack: u32,
    flags: PacketFlag,
    checksum: u16,
    rel_state: u8,
}

/// Returns a module symbol's absolute address.
fn get_module_symbol_address(module: &str, symbol: &str) -> Option<usize> {
    let module = module
        .encode_utf16()
        .chain(iter::once(0))
        .collect::<Vec<u16>>();
    let symbol = CString::new(symbol).unwrap();
    unsafe {
        let handle = GetModuleHandleW(PCWSTR(module.as_ptr() as _)).unwrap();
        match GetProcAddress(handle, PCSTR(symbol.as_ptr() as _)) {
            Some(func) => Some(func as usize),
            None => None,
        }
    }
}

#[no_mangle]
unsafe extern "system" fn DllMain(_hinst: HANDLE, reason: u32, _reserved: *mut c_void) -> BOOL {
    let _ = windows::Win32::System::Console::AllocConsole();

    match reason {
        DLL_PROCESS_ATTACH  => {
            unsafe { main().unwrap() };
        },
    };

    return BOOL::from(true);
}

unsafe fn main() -> Result<(), Box<dyn Error>> {
    if SendtoHook.is_enabled() {
        return Ok(());
    }

    println!("Attaching...");

    let address = get_module_symbol_address("WS2_32.dll", "sendto")
        .expect("could not find 'sendto address");
    let target: FnSendto = mem::transmute(address);

    SendtoHook
        .initialize(target, sendto_detour)?
        .enable()?;

    println!("Attached");

    Ok(())
}

#[derive(Debug)]
struct DataFragment {
    filename: Vec<u8>,
    buffer: Vec<u8>,
    bytes: u32,
    bits: u32,
    is_compressed: bool,
    num_fragments: i32,
    acked_fragments: i32,
}

impl Default for DataFragment {
    fn default() -> Self { 
        Self {
            filename: vec![0; 260],
            buffer: vec![],
            bytes: 0,
            bits: 0,
            is_compressed: false,
            num_fragments: 0,
            acked_fragments: 0,
        }
    }
}

static RECEIVE_LIST: LazyLock<Mutex<[DataFragment; 2]>> = LazyLock::new(|| { 
    Mutex::new([Default::default(), Default::default()]) 
});

fn check_receiving_list(stream: usize) -> bool {
    let data = &mut RECEIVE_LIST.lock().unwrap()[stream];

    if data.buffer.is_empty() {
        // ProcessMesssages without data_buffer
        return true;
    }
    if data.acked_fragments < data.num_fragments {
        // ProcessMessages without data_buffer 
        return true;
    }
    if data.acked_fragments > data.num_fragments {
        // ProcessMessages with data_buffer
        return false;
    }

    if data.is_compressed {
        todo!();
    }

    if data.filename[0] == 0 {
        // ProcessMessages with data_buffer
        let mut reader = BitReader::new(data.buffer[..data.bytes as usize].to_vec());
        if !process_messages(&mut reader) {
            return false;
        }
    } else {
        todo!();
    }

    if !data.buffer.is_empty() {
        data.buffer.clear();
    }

    // ProcessMessages without data_buffer 
    true
}

fn process_control_message(command: u8, reader: &mut BitReader) -> bool {
    // net_NOP
    if command == 0 {
        return true;
    }

    // net_Disconnect
    if command == 1 {
        let reason = reader.read_string();
        println!("Disconnected. Reason: {:?}", reason);
        return false;
    }

    // net_File
    if command == 2 {
        let transfer_id = reader.read_u32(32);
        let string = reader.read_string();

        if reader.read_u8(1) != 0 {
            println!("File requested {:?} {transfer_id}", string);
        } else {
            println!("File denied {:?} {transfer_id}", string);
        }

        return true;
    }

    false
}

fn process_messages(reader: &mut BitReader) -> bool {
    'parse_loop: loop {
        if reader.bits_left() < 6 {
            break;
        }

        let command = reader.read_u8(6);
        if command <= 2 {
            if !process_control_message(command, reader) {
                return false;
            }
            continue;
        }

        //println!("Command {command}");
        //println!("Command content {:02x?}", &reader.content[reader.pos / 8..]); 
        //println!("Bits left {}", reader.bits_left());

        match command {
            NET_NOP => {
            },
            NET_TICK => {
                NETTick::parse(reader);
            },
            NET_STRINGCMD => {
                NETStringCmd::parse(reader);
            },
            NET_SETCONVAR => {
                NETSetConVar::parse(reader);
            },
            NET_SIGNONSTATE => {
                NETSignonState::parse(reader);
            },
            CLC_CLIENTINFO => {
                CLCClientInfo::parse(reader);
            },
            CLC_MOVE => {
                CLCMove::parse(reader);
            },
            CLC_BASELINEACK => {
                CLCBaselineAck::parse(reader);
            },
            CLC_LISTENEVENTS => {
                CLCListenEvents::parse(reader);
            },
            CLC_LOADINGPROGRESS => {
                CLCLoadingProgress::parse(reader);
            },
            CLC_CMDKEYVALUES => {
                CmdKeyValues::parse(reader);
            },
            _ => {
                println!("Command {}", command);
                println!("NOT IMPLEMENTED");
                break 'parse_loop;
            }
        }
    }

    true
}

fn read_sub_channel_data(reader: &mut BitReader, stream: usize) -> bool {
    let data = &mut RECEIVE_LIST.lock().unwrap()[stream];

    let mut start_fragment: i32 = 0;
    let mut num_fragments: i32 = 0;
    let mut offset: u32 = 0;
    let mut length: u32 = 0;

    let single_block: bool = reader.read_u8(1) == 0;

    if !single_block {
        start_fragment = reader.read_u32(18) as i32;
        num_fragments = reader.read_u8(3) as i32;
        offset = (start_fragment * (1 << 8)) as u32;
        length = (num_fragments * (1 << 8)) as u32;
    }

    if offset == 0 {
        data.filename[0] = 0;
        data.is_compressed = false;

        if single_block {
            // Check if the data is compressed
            if reader.read_u8(1) == 1 {
                data.is_compressed = true;
                let _ = reader.read_u32(26);
            }
            // L4D2
            data.bytes = reader.read_u32(18);
            // L4D1
            //data.bytes = reader.read_u32(17);
        } else {
            if reader.read_u8(1) == 1 {
                let _ = reader.read_u32(32);
                let filename = reader.read_string();
                let filename = filename.to_bytes();
                data.filename[..filename.len()].copy_from_slice(filename);
            }

            if reader.read_u8(1) == 1 {
                data.is_compressed = true;
                let _ = reader.read_u32(26);
            }
            data.bytes = reader.read_u32(26);
        }

        if !data.buffer.is_empty() {
            data.buffer.clear();
        }

        data.bits = data.bytes * 8;
        data.buffer = vec![0; ((((data.bytes) + (4-1)) / 4) * 4) as usize];
        data.num_fragments = ((data.bytes + (1 << 8) - 1) / (1 << 8)) as i32;
        data.acked_fragments = 0;

        if single_block {
            num_fragments = data.num_fragments as i32; 
            length = (num_fragments * (1 << 8)) as u32;
        }
    } else {
        if data.buffer.is_empty() {
            return false;
        }
    }

    if start_fragment + num_fragments == data.num_fragments as i32 {
        let rest = (1 << 8) - (data.bytes % (1 << 8));
        if rest < (1 << 8) {
            length -= rest;
        }
    }

    assert!(offset + length <= data.bytes);

    // buf.ReadBytes
    for i in 0..length {
        *data.buffer.get_mut((offset + i) as usize).unwrap() = reader.read_u8(8);
    }

    data.acked_fragments += num_fragments;
    true
}

fn sendto_detour(s: SOCKET, buf: *mut c_char, len: c_int, flags: c_int, to: *mut SOCKADDR, tolen: c_int) -> c_int {
    let packet: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, len as usize) };
    let header: NetPacketHeader = unsafe { std::ptr::read(packet.as_ptr() as _) };

    // CONNECTIONLESS_HEADER
    if packet[..4] == [0xff, 0xff, 0xff, 0xff] {
        return unsafe { SendtoHook.call(s, buf, len, flags, to, tolen) };
    }


    let content;
    if header.flags.0 & PACKET_FLAG_CHOKED != 0 {
        // Chocked packet
        content = &packet[std::mem::size_of::<NetPacketHeader>() + 1..];
    } else {
        content = &packet[std::mem::size_of::<NetPacketHeader>()..];
    }
    
    let mut reader = BitReader::new(content.to_vec());

    // Read subchannel data
    if header.flags.0 & PACKET_FLAG_RELIABLE != 0 {
        let bit = 1 << reader.read_u8(3);

        for i in 0..2 {
            if reader.read_u8(1) != 0 {
                if !read_sub_channel_data(&mut reader, i) {
                    return unsafe { SendtoHook.call(s, buf, len, flags, to, tolen) }
                }
            }
        }

        for i in 0..2 {
            if !check_receiving_list(i) {
                return unsafe { SendtoHook.call(s, buf, len, flags, to, tolen) }
            }
        }
    }

    if reader.bits_left() > 0 {
        if !process_messages(&mut reader) {
            return unsafe { SendtoHook.call(s, buf, len, flags, to, tolen) }
        }
    } else {
        println!("No bits left");
    }

    unsafe { SendtoHook.call(s, buf, len, flags, to, tolen) }
}
