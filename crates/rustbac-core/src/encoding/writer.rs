use crate::EncodeError;

#[derive(Debug)]
pub struct Writer<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> Writer<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub const fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub fn as_written(&self) -> &[u8] {
        &self.buf[..self.pos]
    }

    pub fn write_u8(&mut self, value: u8) -> Result<(), EncodeError> {
        if self.remaining() < 1 {
            return Err(EncodeError::BufferTooSmall);
        }
        self.buf[self.pos] = value;
        self.pos += 1;
        Ok(())
    }

    pub fn write_all(&mut self, data: &[u8]) -> Result<(), EncodeError> {
        if self.remaining() < data.len() {
            return Err(EncodeError::BufferTooSmall);
        }
        let end = self.pos + data.len();
        self.buf[self.pos..end].copy_from_slice(data);
        self.pos = end;
        Ok(())
    }

    pub fn write_be_u16(&mut self, value: u16) -> Result<(), EncodeError> {
        self.write_all(&value.to_be_bytes())
    }

    pub fn write_be_u32(&mut self, value: u32) -> Result<(), EncodeError> {
        self.write_all(&value.to_be_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::Writer;
    use crate::EncodeError;

    #[test]
    fn writer_writes_values() {
        let mut buf = [0u8; 4];
        let mut w = Writer::new(&mut buf);
        w.write_u8(1).unwrap();
        w.write_all(&[2, 3]).unwrap();
        assert_eq!(w.as_written(), &[1, 2, 3]);
    }

    #[test]
    fn writer_bounds() {
        let mut buf = [0u8; 1];
        let mut w = Writer::new(&mut buf);
        w.write_u8(1).unwrap();
        assert_eq!(w.write_u8(2).unwrap_err(), EncodeError::BufferTooSmall);
    }
}
