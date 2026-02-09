#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ApduType {
    ConfirmedRequest = 0,
    UnconfirmedRequest = 1,
    SimpleAck = 2,
    ComplexAck = 3,
    SegmentAck = 4,
    Error = 5,
    Reject = 6,
    Abort = 7,
}

impl ApduType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::ConfirmedRequest),
            1 => Some(Self::UnconfirmedRequest),
            2 => Some(Self::SimpleAck),
            3 => Some(Self::ComplexAck),
            4 => Some(Self::SegmentAck),
            5 => Some(Self::Error),
            6 => Some(Self::Reject),
            7 => Some(Self::Abort),
            _ => None,
        }
    }
}
