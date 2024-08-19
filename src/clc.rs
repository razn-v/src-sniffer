use std::ffi::CString;
use std::sync::{Mutex, LazyLock};

use crate::BitReader;

#[derive(Debug, Default)]
pub struct CUserCmd {
    command_number: i32,
    tick_count: i32,
    viewangles: QAngle,
    forwardmove: f32,
    sidemove: f32,
    upmove: f32,
    buttons: i32,
    impulse: u8,
    weaponselect: i32,
    weaponsubtype: i32,
    //random_seed: i32,
    mousedx: i16,
    mousedy: i16,
    hasbeenpredicted: bool
}

#[derive(Debug, Default)]
pub struct QAngle {
    x: f32,
    y: f32,
    z: f32
}

pub const NET_NOP: u8 = 0;
pub const NET_TICK: u8 = 4;
pub const NET_STRINGCMD: u8 = 5;
pub const NET_SETCONVAR: u8 = 6;
pub const NET_SIGNONSTATE: u8 = 7;

pub const CLC_CLIENTINFO: u8 = 8;
pub const CLC_MOVE: u8 = 9;
pub const CLC_BASELINEACK: u8 = 11;
pub const CLC_LISTENEVENTS: u8 = 12;
pub const CLC_LOADINGPROGRESS: u8 = 16;
pub const CLC_CMDKEYVALUES: u8 = 18;

#[derive(Debug, Default)]
pub struct NETTick {
    n_tick: i32,
    fl_host_frame_time: f32,
    fl_host_frame_time_std_deviation: f32
}

impl NETTick {
    pub fn parse(reader: &mut BitReader) {
        let n_tick = reader.read_u32(32) as i32;
        let fl_host_frame_time = reader.read_u16(16) as f32 / 100000.0;
        let fl_host_frame_time_std_deviation = reader.read_u16(16) as f32 / 100000.0;

        println!("{:?}", NETTick {
            n_tick,
            fl_host_frame_time,
            fl_host_frame_time_std_deviation
        });
    }
}

#[derive(Debug, Default)]
pub struct CLCMove {
    n_new_commands: u8,
    n_backup_commands: u8,
    n_length: u16,
    user_cmd: CUserCmd
}

static LAST_MOVE: LazyLock<Mutex<CLCMove>> = LazyLock::new(|| { Mutex::new(Default::default()) });

impl CLCMove {
    pub fn parse(reader: &mut BitReader) {
        let n_new_commands = reader.read_u8(4);
        let n_backup_commands = reader.read_u8(3);
        // Length in bits
        let n_length = reader.read_u16(16);

        let mut buf = Vec::new();
        for i in 0..n_length {
            buf.push(reader.read_u8(1));
        }
        let mut reader = BitReader::new(buf);

        let mut from = (*LAST_MOVE).lock().unwrap();

        // ReadUsercmd
        let mut user_cmd = CUserCmd::default();
        if reader.read_u8(1) == 1 {
            user_cmd.command_number = reader.read_u32(32) as i32;
        } else {
            user_cmd.command_number = from.user_cmd.command_number + 1;
        }

        if reader.read_u8(1) == 1 {
            user_cmd.tick_count = reader.read_u32(32) as i32;
        } else {
            user_cmd.tick_count = from.user_cmd.tick_count + 1; 
        }

        // Read direction
        if reader.read_u8(1) == 1 {
            user_cmd.viewangles.x = reader.read_u32(32) as f32;
        }
        if reader.read_u8(1) == 1 {
            user_cmd.viewangles.y = reader.read_u32(32) as f32;
        }
        if reader.read_u8(1) == 1 {
            user_cmd.viewangles.z = reader.read_u32(32) as f32;
        }

        // Read movement
        if reader.read_u8(1) == 1 {
            user_cmd.forwardmove = reader.read_u32(32) as f32;
        }
        if reader.read_u8(1) == 1 {
            user_cmd.sidemove = reader.read_u32(32) as f32;
        }
        if reader.read_u8(1) == 1 {
            user_cmd.upmove = reader.read_u32(32) as f32;
        }

        // Read buttons
        if reader.read_u8(1) == 1 {
            user_cmd.buttons = reader.read_u32(32) as i32;
        }
        if reader.read_u8(1) == 1 {
            user_cmd.impulse = reader.read_u8(8);
        }

        if reader.read_u8(1) == 1 {
            user_cmd.weaponselect = reader.read_u16(11) as i32;
            if reader.read_u8(1) == 1 {
                user_cmd.weaponsubtype = reader.read_u8(6) as i32;
            }
        }

        if reader.read_u8(1) == 1 {
            user_cmd.mousedx = reader.read_u16(16) as i16;
        }
        if reader.read_u8(1) == 1 {
            user_cmd.mousedy = reader.read_u16(16) as i16;
        }

        let new_move = CLCMove {
            n_new_commands,
            n_backup_commands,
            n_length,
            user_cmd
        };

        println!("{:?}", new_move);
        *from = new_move;
    }
}


#[derive(Debug)]
pub struct CLCClientInfo<'a> {
    n_server_count: i32,
    n_send_table_crc: u32,
    b_is_hltv: bool,
    n_friends_id: u32,
    friends_name: CString,
    n_custom_files: &'a [u32; 4]
}

impl<'a> CLCClientInfo<'a> {
    pub fn parse(reader: &mut BitReader) {
        let n_server_count = reader.read_u32(32) as i32;
        let n_send_table_crc = reader.read_u32(32);
        let b_is_hltv = reader.read_u8(1) == 1;
        let n_friends_id = reader.read_u32(32);
        let friends_name = reader.read_string(); 

        let mut n_custom_files = [0; 4];
        for i in 0..4 {
            if reader.read_u8(1) != 0 {
                n_custom_files[i] = reader.read_u32(32);
            } else {
                n_custom_files[i] = 0;
            }
        }

        println!("{:?}", CLCClientInfo {
            n_server_count,
            n_send_table_crc,
            b_is_hltv,
            n_friends_id,
            friends_name,
            n_custom_files: &n_custom_files
        });
    }
}

#[derive(Debug)]
struct ConVar {
    name: CString,
    value: CString,
}

#[derive(Debug)]
pub struct NETSetConVar {
    convars: Vec<ConVar>
}

impl NETSetConVar {
    pub fn parse(reader: &mut BitReader) {
        let numvars = reader.read_u8(8);
        let mut convars: Vec<ConVar> = Vec::new();

        for i in 0..numvars {
            convars.push(ConVar {
                name: reader.read_string(),
                value: reader.read_string()
            });
        }

        println!("{:?}", NETSetConVar {
            convars
        });
    }
}

const TYPE_NONE: u8 = 0;
const TYPE_STRING: u8 = 1;
const TYPE_INT: u8 = 2;
const TYPE_FLOAT: u8 = 3;
const TYPE_PTR: u8 = 4;
const TYPE_WSTRING: u8 = 5;
const TYPE_COLOR: u8 = 6;
const TYPE_UINT64: u8 = 7;
const TYPE_NUMTYPES: u8 = 8;

pub struct CmdKeyValues {

}

impl CmdKeyValues {
    pub fn parse(reader: &mut BitReader) {
        let num_bytes = reader.read_u32(32);

        let mut buffer: Vec<u8> = Vec::new();
        for i in 0..num_bytes {
            buffer.push(reader.read_u8(8));
        }

        let mut reader = BitReader::new(buffer);
        let mut peer_type = reader.read_u8(8); 

        loop {
            if peer_type == 11 {
                break;
            }

            let token = reader.read_string();

            println!("Token {:?}", token);

            match peer_type {
                TYPE_NONE => println!("None value"),
                TYPE_STRING => {
                    let value = reader.read_string();
                    println!("String value {:?}", value);
                },
                TYPE_WSTRING => {
                    println!("WString"); 
                },
                TYPE_INT => {
                    let value = reader.read_u32(32) as i32;
                    println!("Int value {}", value);
                },
                TYPE_UINT64 => {
                    let value = reader.read_u64(64);
                    println!("UInt64 value {}", value);
                },
                TYPE_FLOAT => {
                    let value = reader.read_u32(32) as f32;
                    println!("Float value {}", value);
                },
                TYPE_COLOR => {
                    let r = reader.read_u8(8);
                    let g = reader.read_u8(8);
                    let b = reader.read_u8(8);
                    let a = reader.read_u8(8);
                    println!("R: {} G:{} B:{} A:{}", r, g, b, a);
                },
                TYPE_PTR => {
                    let value = reader.read_u32(32);
                    println!("Ptr value {}", value);
                },
                _ => unreachable!()
            }

            peer_type = reader.read_u8(8); 
        }
    }
}

#[derive(Debug)]
pub struct NETSignonState {
    n_signon_state: u8,
    n_spawn_count: u32,
    idk1: u32,
    idk2_len: u32,
    idk2_buf: Vec<u8>,
    idk3_len: u32,
    idk3_buf: Vec<u8>
}

impl NETSignonState {
    pub fn parse(reader: &mut BitReader) {
        let n_signon_state = reader.read_u8(8);
        let n_spawn_count = reader.read_u32(32);
        let idk1 = reader.read_u32(32);

        let idk2_len = reader.read_u32(32);
        let mut idk2_buf = Vec::new();
        if idk2_len > 0 {
            for i in 0..idk2_len {
                idk2_buf.push(reader.read_u8(8));
            }
        }

        let idk3_len = reader.read_u32(32);
        let mut idk3_buf = Vec::new();
        if idk3_len > 0 {
            for i in 0..idk3_len {
                idk3_buf.push(reader.read_u8(8));
            }
        }

        println!("{:x?}", NETSignonState {
            n_signon_state,
            n_spawn_count,
            idk1,
            idk2_len,
            idk2_buf,
            idk3_len,
            idk3_buf
        });
    }
}

#[derive(Debug)]
pub struct CLCListenEvents {
    events: Vec<u32>
}

impl CLCListenEvents {
    pub fn parse(reader: &mut BitReader) {
        let mut events = Vec::new();
        for i in 0..16 {
            events.push(reader.read_u32(32));
        }
        println!("{:x?}", CLCListenEvents {
            events
        });
    }
}

#[derive(Debug)]
pub struct NETStringCmd {
    command: CString
}

impl NETStringCmd {
    pub fn parse(reader: &mut BitReader) {
        println!("{:x?}", NETStringCmd {
            command: reader.read_string()
        });
    }
}

#[derive(Debug)]
pub struct CLCBaselineAck {
    n_baseline_tick: u32,
    n_baseline_nr: u32
}

impl CLCBaselineAck {
    pub fn parse(reader: &mut BitReader) {
        println!("{:x?}", Self {
            n_baseline_tick: reader.read_u32(32),
            n_baseline_nr: reader.read_u32(1)
        });
    }
}

#[derive(Debug)]
pub struct CLCLoadingProgress {
    idk: u8
}

impl CLCLoadingProgress {
    pub fn parse(reader: &mut BitReader) {
        println!("{:?}", Self {
            idk: reader.read_u8(8),
        });
    }
}
