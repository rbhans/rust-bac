use crate::types::{BitString, Date, ObjectId, Time};

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
