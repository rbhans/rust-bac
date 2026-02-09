use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    BufferTooSmall,
    ValueOutOfRange,
    InvalidLength,
    Unsupported,
    Message(&'static str),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall => f.write_str("buffer too small"),
            Self::ValueOutOfRange => f.write_str("value out of range"),
            Self::InvalidLength => f.write_str("invalid length"),
            Self::Unsupported => f.write_str("operation unsupported"),
            Self::Message(msg) => f.write_str(msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedEof,
    InvalidTag,
    InvalidLength,
    InvalidValue,
    Unsupported,
    Message(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::InvalidTag => f.write_str("invalid tag"),
            Self::InvalidLength => f.write_str("invalid length"),
            Self::InvalidValue => f.write_str("invalid value"),
            Self::Unsupported => f.write_str("operation unsupported"),
            Self::Message(msg) => f.write_str(msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}
