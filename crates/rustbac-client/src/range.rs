use crate::ClientDataValue;
use rustbac_core::types::{ObjectId, PropertyId};

#[derive(Debug, Clone, PartialEq)]
pub struct ClientBitString {
    pub unused_bits: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadRangeResult {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub result_flags: ClientBitString,
    pub item_count: u32,
    pub items: Vec<ClientDataValue>,
}
