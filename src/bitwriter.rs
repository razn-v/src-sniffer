use std::ffi::CString;

pub struct BitWriter {
    pub content: Vec<u8>,
    pub pos: usize
}

impl BitWriter {
    pub fn new(content: Vec<u8>) -> Self {
        Self {
            content,
            pos: 0
        }
    }

    // Write at most 8 bits
    #[track_caller]
    pub fn write_u8(&mut self, content: u8, bits: usize) {
        assert!(bits <= 8);

        // Calculate the byte position in the buffer
        let byte_pos = (self.pos as f64 / 8.0).floor() as usize;
        // Bit position in the byte
        let bit_pos = self.pos % 8;
        
        if byte_pos >= self.content.len() {
            self.content.push(0);
        }
        
        // Check if we have to write through 2 different parts
        if bit_pos + bits > 8 {
            if byte_pos + 1 >= self.content.len() {
                self.content.push(0);
            }
            
            // Write first part
            let p1_len =  8 - bit_pos;
            let p1 = content & ((1 << p1_len) - 1);
            self.content[byte_pos] = (p1 << bit_pos) | self.content[byte_pos];
            
            // Write second part
            let p2_len = bits - p1_len;
            let p2 = (content >> p1_len) & ((1 << p2_len) - 1);
            self.content[byte_pos + 1] = p2 | self.content[byte_pos + 1];
        } else {
            self.content[byte_pos] = (content << bit_pos) | self.content[byte_pos];
        }

        self.pos += bits;
    }
    
    // Write at most 16 bits
    #[track_caller]
    pub fn write_u16(&mut self, content: u16, bits: usize) {
        assert!(bits <= 16);
        
        if bits <= 8 {
            self.write_u8(content as u8, bits);
            return;
        }
        
        // Write the first and second part
        self.write_u8((content & !(1 << 8)) as u8, 8);
        self.write_u8((content >> 8) as u8, bits - 8);
    }
    
    // Write at most 32 bits
    #[track_caller]
    pub fn write_u32(&mut self, content: u32, bits: usize) {
        assert!(bits <= 32);
        
        if bits <= 16 {
            self.write_u16(content as u16, bits);
            return;
        }
        
        // Write the first and second part
        self.write_u16((content & !(1 << 16)) as u16, 16);
        self.write_u16((content >> 16) as u16, bits - 16);
    }

    // Write at most 64 bits
    #[track_caller]
    pub fn write_u64(&mut self, content: u64, bits: usize) {
        assert!(bits <= 64);
        
        if bits <= 32 {
            self.write_u32(content as u32, bits);
            return;
        }
        
        // Write the first and second part
        self.write_u32((content & !(1 << 32)) as u32, 32);
        self.write_u32((content >> 32) as u32, bits - 32);
    }
    
    #[track_caller]
    pub fn write_string(&mut self, string: CString) {
        let content = string.as_bytes_with_nul();
        
        for byte in content {
            self.write_u8(*byte, 8);
        }
    }
}
