#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitString<'a> {
    pub unused_bits: u8,
    pub data: &'a [u8],
}

impl<'a> BitString<'a> {
    pub const fn new(unused_bits: u8, data: &'a [u8]) -> Self {
        Self { unused_bits, data }
    }
}
