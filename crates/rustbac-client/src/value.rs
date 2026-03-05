use rustbac_core::types::{Date, Time};

/// An owned BACnet application-data value returned by client read operations.
///
/// This is the client-side counterpart to the zero-copy `DataValue<'_>` used internally.
/// All byte-slice and string fields are allocated as owned `Vec`/`String` so the value
/// can outlive the receive buffer.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClientDataValue {
    /// BACnet Null — no value present.
    Null,
    /// BACnet Boolean.
    Boolean(bool),
    /// BACnet Unsigned Integer (32-bit).
    Unsigned(u32),
    /// BACnet Signed Integer (32-bit).
    Signed(i32),
    /// BACnet Real (IEEE 754 single-precision float).
    Real(f32),
    /// BACnet Double (IEEE 754 double-precision float).
    Double(f64),
    /// BACnet Octet String — arbitrary raw bytes.
    OctetString(Vec<u8>),
    /// BACnet Character String — UTF-8 text.
    CharacterString(String),
    /// BACnet Bit String.
    ///
    /// `unused_bits` is the number of padding bits in the last byte of `data` that are not
    /// part of the logical bit string (0–7).
    BitString { unused_bits: u8, data: Vec<u8> },
    /// BACnet Enumerated value — the raw numeric discriminant.
    Enumerated(u32),
    /// BACnet Date.
    Date(Date),
    /// BACnet Time.
    Time(Time),
    /// BACnet ObjectIdentifier — encodes both the object type and instance number.
    ObjectId(rustbac_core::types::ObjectId),
    /// A constructed (complex) value containing a sequence of child values.
    ///
    /// `tag_num` is the context tag number of the opening/closing tag pair.
    Constructed {
        tag_num: u8,
        values: Vec<ClientDataValue>,
    },
}
