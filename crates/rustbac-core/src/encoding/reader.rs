use crate::DecodeError;

#[derive(Debug, Clone, Copy)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub const fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn peek_u8(&self) -> Result<u8, DecodeError> {
        self.buf
            .get(self.pos)
            .copied()
            .ok_or(DecodeError::UnexpectedEof)
    }

    pub fn read_u8(&mut self) -> Result<u8, DecodeError> {
        let byte = self.peek_u8()?;
        self.pos += 1;
        Ok(byte)
    }

    pub fn read_exact(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < len {
            return Err(DecodeError::UnexpectedEof);
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.buf[start..start + len])
    }

    pub fn read_be_u16(&mut self) -> Result<u16, DecodeError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_be_u32(&mut self) -> Result<u32, DecodeError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;
    use crate::DecodeError;

    #[test]
    fn reader_reads_values() {
        let mut r = Reader::new(&[1, 2, 3, 4, 5]);
        assert_eq!(r.read_u8().unwrap(), 1);
        assert_eq!(r.read_exact(2).unwrap(), &[2, 3]);
        assert_eq!(r.remaining(), 2);
    }

    #[test]
    fn reader_bounds() {
        let mut r = Reader::new(&[1]);
        assert_eq!(r.read_u8().unwrap(), 1);
        assert_eq!(r.read_u8().unwrap_err(), DecodeError::UnexpectedEof);
    }
}
