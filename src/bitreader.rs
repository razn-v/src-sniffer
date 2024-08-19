use std::ffi::CString;

pub struct BitReader {
    pub content: Vec<u8>,
    // Bit position in the buffer
    pub pos: usize
}

impl BitReader {
    pub fn new(content: Vec<u8>) -> Self {
        Self {
            content,
            pos: 0
        }
    }

    pub fn is_empty(&self) -> bool {
        (self.pos as f64 / 8.).ceil() as usize >= self.content.len()
    }

    pub fn bits_left(&self) -> usize {
        self.content.len() * 8 - self.pos
    }

    // Read at most 8 bits
    #[track_caller]
    pub fn read_u8(&mut self, bits: usize) -> u8 {
        assert!(bits <= 8);

        // Calculate the byte position in the buffer
        let byte_pos = (self.pos as f64 / 8.0).floor() as usize;
        // Bit position in the byte
        let bit_pos = self.pos % 8;
        
        let read;
        // Check if we have to read through 2 different parts
        if bit_pos + bits > 8 {
            // Read the first part
            let p1_len =  8 - bit_pos;
            let p1 = (self.content[byte_pos] >> bit_pos) & (2u8.pow(p1_len as u32) - 1);
            
            // Read the second part
            let p2_len = bits - p1_len;
            let p2 = self.content[byte_pos + 1] & (2u8.pow(p2_len as u32) - 1);
            
            // Combine both part
            read = (p2 << p1_len) | p1;
        } else {
            // Read the corresponding bits
            read = (self.content[byte_pos] >> bit_pos) & ((2u64.pow(bits as u32) - 1) as u8);
        }

        self.pos += bits;
        read
    }
    
    // Read at most 16 bits
    #[track_caller]
    pub fn read_u16(&mut self, bits: usize) -> u16 {
        assert!(bits <= 16);
        
        if bits <= 8 {
            return self.read_u8(bits) as u16;
        }
        
        // Read the first and second part
        let p1 = self.read_u8(8) as u16;
        let p2 = self.read_u8(bits - 8) as u16;

        // Combine both part and return the result
        (p2 << 8) | p1
    }
    
    // Read at most 32 bits
    #[track_caller]
    pub fn read_u32(&mut self, bits: usize) -> u32 {
        assert!(bits <= 32);
        
        if bits <= 16 {
            return self.read_u16(bits) as u32;
        }
        
        // Read the first and second part
        let p1 = self.read_u16(16) as u32;
        let p2 = self.read_u16(bits - 16) as u32;

        // Combine both part and return the result
        (p2 << 16) | p1
    }

    // Read at most 64 bits
    #[track_caller]
    pub fn read_u64(&mut self, bits: usize) -> u64 {
        assert!(bits <= 64);
        
        if bits <= 32 {
            return self.read_u32(bits) as u64;
        }
        
        // Read the first and second part
        let p1 = self.read_u32(32) as u64;
        let p2 = self.read_u32(bits - 32) as u64;

        // Combine both part and return the result
        (p2 << 32) | p1
    }

    #[track_caller]
    pub fn read_string(&mut self) -> CString {
        let mut byte = 1;
        
        let mut res = Vec::new();
        while byte != 0 {
            byte = self.read_u8(8);
            res.push(byte);
        }

        CString::from_vec_with_nul(res).unwrap()
    }
}
