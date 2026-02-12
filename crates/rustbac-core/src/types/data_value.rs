use crate::types::{BitString, Date, ObjectId, Time};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// A BACnet application-layer data value decoded from the wire.
///
/// Borrows byte-level data (octet strings, character strings, bit strings)
/// from the input buffer to avoid allocation.
///
/// The [`Constructed`](Self::Constructed) variant (available with the `alloc`
/// feature) holds a sequence of child values decoded from an opening/closing
/// context-tag pair. This is used for complex properties such as weekly
/// schedules and calendar entries.
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
    /// A constructed (complex) value containing a sequence of child values.
    ///
    /// The `tag_num` is the context tag number from the opening/closing pair.
    #[cfg(feature = "alloc")]
    Constructed {
        tag_num: u8,
        values: Vec<DataValue<'a>>,
    },
}
