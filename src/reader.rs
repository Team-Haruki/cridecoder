//! Binary reader utilities with endianness support

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io::{self, Read, Seek, SeekFrom};

/// Reader wrapper with typed read methods
pub struct Reader<R: Read + Seek> {
    inner: R,
}

impl<R: Read + Seek> Reader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }

    pub fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }

    pub fn stream_position(&mut self) -> io::Result<u64> {
        self.inner.stream_position()
    }

    // Big-endian reads
    pub fn read_u8(&mut self) -> io::Result<u8> {
        self.inner.read_u8()
    }

    pub fn read_i8(&mut self) -> io::Result<i8> {
        self.inner.read_i8()
    }

    pub fn read_u16(&mut self) -> io::Result<u16> {
        self.inner.read_u16::<BigEndian>()
    }

    pub fn read_i16(&mut self) -> io::Result<i16> {
        self.inner.read_i16::<BigEndian>()
    }

    pub fn read_u32(&mut self) -> io::Result<u32> {
        self.inner.read_u32::<BigEndian>()
    }

    pub fn read_i32(&mut self) -> io::Result<i32> {
        self.inner.read_i32::<BigEndian>()
    }

    pub fn read_u64(&mut self) -> io::Result<u64> {
        self.inner.read_u64::<BigEndian>()
    }

    pub fn read_f32(&mut self) -> io::Result<f32> {
        self.inner.read_f32::<BigEndian>()
    }

    // Little-endian reads
    pub fn read_u16_le(&mut self) -> io::Result<u16> {
        self.inner.read_u16::<LittleEndian>()
    }

    pub fn read_u32_le(&mut self) -> io::Result<u32> {
        self.inner.read_u32::<LittleEndian>()
    }

    /// Read exact number of bytes
    pub fn read_bytes(&mut self, n: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Read bytes at a specific offset, then restore position
    pub fn read_bytes_at(&mut self, n: usize, offset: u64) -> io::Result<Vec<u8>> {
        let pos = self.inner.stream_position()?;
        self.inner.seek(SeekFrom::Start(offset))?;
        let result = self.read_bytes(n);
        self.inner.seek(SeekFrom::Start(pos))?;
        result
    }

    /// Read null-terminated string
    pub fn read_string0(&mut self) -> io::Result<String> {
        let mut buf = Vec::new();
        loop {
            let b = self.inner.read_u8()?;
            if b == 0 {
                break;
            }
            buf.push(b);
        }
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Read null-terminated string at offset, then restore position
    pub fn read_string0_at(&mut self, offset: u64) -> io::Result<String> {
        let pos = self.inner.stream_position()?;
        self.inner.seek(SeekFrom::Start(offset))?;
        let result = self.read_string0();
        self.inner.seek(SeekFrom::Start(pos))?;
        result
    }
}

/// Calculate alignment
pub fn align(alignment: u32, offset: u32) -> u32 {
    if alignment == 0 {
        return offset;
    }
    offset.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_u32() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let mut reader = Reader::new(Cursor::new(data));
        assert_eq!(reader.read_u32().unwrap(), 0x12345678);
    }

    #[test]
    fn test_read_string0() {
        let data = b"hello\0world";
        let mut reader = Reader::new(Cursor::new(data.to_vec()));
        assert_eq!(reader.read_string0().unwrap(), "hello");
    }

    #[test]
    fn test_align() {
        assert_eq!(align(4, 0), 0);
        assert_eq!(align(4, 1), 4);
        assert_eq!(align(4, 4), 4);
        assert_eq!(align(4, 5), 8);
        assert_eq!(align(32, 100), 128);
    }
}
