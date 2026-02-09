use rustbac_core::types::{Date, Time};

#[derive(Debug, Clone, PartialEq)]
pub enum ClientDataValue {
    Null,
    Boolean(bool),
    Unsigned(u32),
    Signed(i32),
    Real(f32),
    Double(f64),
    OctetString(Vec<u8>),
    CharacterString(String),
    BitString { unused_bits: u8, data: Vec<u8> },
    Enumerated(u32),
    Date(Date),
    Time(Time),
    ObjectId(rustbac_core::types::ObjectId),
}
