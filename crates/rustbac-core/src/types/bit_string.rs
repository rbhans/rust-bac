/// A BACnet bit string, borrowing its raw bytes from the input buffer.
///
/// `unused_bits` indicates how many trailing bits in the last byte of `data`
/// are padding and should be ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitString<'a> {
    /// Number of unused trailing bits in the last byte of `data`.
    pub unused_bits: u8,
    /// The raw bytes holding the bit string payload.
    pub data: &'a [u8],
}

impl<'a> BitString<'a> {
    pub const fn new(unused_bits: u8, data: &'a [u8]) -> Self {
        Self { unused_bits, data }
    }
}
