use crate::types::{BitString, Date, ObjectId, Time};

/// A BACnet application-layer data value decoded from the wire.
///
/// Borrows byte-level data (octet strings, character strings, bit strings)
/// from the input buffer to avoid allocation.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue<'a> {
    Null,
    Boolean(bool),
    Unsigned(u32),
    Signed(i32),
    Real(f32),
    Double(f64),
    OctetString(&'a [u8]),
    CharacterString(&'a str),
    BitString(BitString<'a>),
    Enumerated(u32),
    Date(Date),
    Time(Time),
    ObjectId(ObjectId),
}
