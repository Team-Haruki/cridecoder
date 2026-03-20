//! Bit-level reader for HCA decoding

#![allow(dead_code)]

/// Bit reader for HCA data
pub struct BitReader<'a> {
    data: &'a [u8],
    position: usize, // Current bit position
}

impl<'a> BitReader<'a> {
    /// Create a new BitReader
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// Create BitReader starting at byte offset
    pub fn with_offset(data: &'a [u8], byte_offset: usize) -> Self {
        Self {
            data,
            position: byte_offset * 8,
        }
    }

    /// Get current bit position
    pub fn position(&self) -> usize {
        self.position
    }

    /// Set bit position
    pub fn set_position(&mut self, pos: usize) {
        self.position = pos;
    }

    /// Check if reading a single bit at a position
    #[inline]
    fn check_bit(&self, bit_pos: usize) -> u32 {
        let byte_idx = bit_pos / 8;
        let bit_idx = 7 - (bit_pos % 8);
        if byte_idx < self.data.len() {
            ((self.data[byte_idx] >> bit_idx) & 1) as u32
        } else {
            0
        }
    }

    /// Peek at bits without advancing position
    pub fn peek(&self, bits: usize) -> u32 {
        if bits == 0 || bits > 32 {
            return 0;
        }

        let mut value: u32 = 0;
        for i in 0..bits {
            value = (value << 1) | self.check_bit(self.position + i);
        }
        value
    }

    /// Read bits and advance position
    pub fn read(&mut self, bits: usize) -> u32 {
        let value = self.peek(bits);
        self.position += bits;
        value
    }

    /// Skip bits
    pub fn skip(&mut self, bits: usize) {
        self.position += bits;
    }

    /// Read a single bit
    #[inline]
    pub fn read_bit(&mut self) -> bool {
        let result = self.check_bit(self.position) != 0;
        self.position += 1;
        result
    }

    /// Remaining bits available
    pub fn remaining_bits(&self) -> usize {
        let total_bits = self.data.len() * 8;
        if self.position >= total_bits {
            0
        } else {
            total_bits - self.position
        }
    }

    /// Check if there are enough bits remaining
    pub fn has_bits(&self, bits: usize) -> bool {
        self.remaining_bits() >= bits
    }

    /// Read signed value (with leading bit as sign)
    pub fn read_signed(&mut self, bits: usize) -> i32 {
        if bits == 0 {
            return 0;
        }
        let value = self.read(bits) as i32;
        // Sign extend if highest bit is set
        let sign_bit = 1 << (bits - 1);
        if value & sign_bit != 0 {
            value | ((-1i32) << bits)
        } else {
            value
        }
    }

    /// Read variable-length value with scale bits encoding
    /// Returns (scale_count, base_resolution)
    pub fn read_scale_count(&mut self) -> (u32, u32) {
        let delta_bits = self.read(3);
        if delta_bits == 0 {
            return (0, 0);
        }

        let count = self.read(delta_bits as usize);
        let resolution = self.read(4);
        (count + 1, resolution)
    }

    /// Read off-by-one encoded value
    /// Used in HCA scale factor decoding
    pub fn read_off_by_one(&mut self, bits: usize) -> u32 {
        let value = self.read(bits);
        if value != 0 {
            value + 1
        } else {
            0
        }
    }
}

/// Bit writer for HCA encoding (if needed)
pub struct BitWriter {
    data: Vec<u8>,
    position: usize,
}

impl BitWriter {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0; capacity],
            position: 0,
        }
    }

    pub fn write(&mut self, value: u32, bits: usize) {
        if bits == 0 || bits > 32 {
            return;
        }

        for i in 0..bits {
            let bit = (value >> (bits - 1 - i)) & 1;
            let byte_idx = self.position / 8;
            let bit_idx = 7 - (self.position % 8);

            if byte_idx < self.data.len() {
                if bit != 0 {
                    self.data[byte_idx] |= 1 << bit_idx;
                } else {
                    self.data[byte_idx] &= !(1 << bit_idx);
                }
            }
            self.position += 1;
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bits() {
        let data = [0b10110100, 0b01101001];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read(1), 1); // 1
        assert_eq!(reader.read(1), 0); // 0
        assert_eq!(reader.read(2), 0b11); // 11
        assert_eq!(reader.read(4), 0b0100); // 0100
        assert_eq!(reader.read(4), 0b0110); // 0110
    }

    #[test]
    fn test_peek() {
        let data = [0b10110100];
        let reader = BitReader::new(&data);

        assert_eq!(reader.peek(4), 0b1011);
        assert_eq!(reader.peek(8), 0b10110100);
    }

    #[test]
    fn test_skip() {
        let data = [0b10110100, 0b01101001];
        let mut reader = BitReader::new(&data);

        reader.skip(4);
        assert_eq!(reader.read(4), 0b0100);
    }

    #[test]
    fn test_read_signed() {
        let data = [0b11110000];
        let mut reader = BitReader::new(&data);

        // 4 bits: 1111 = -1 when signed
        assert_eq!(reader.read_signed(4), -1);
    }

    #[test]
    fn test_bit_writer() {
        let mut writer = BitWriter::new(2);
        writer.write(0b1011, 4);
        writer.write(0b0100, 4);

        assert_eq!(writer.data()[0], 0b10110100);
    }
}
