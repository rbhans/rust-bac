use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{decode_unsigned, encode_ctx_object_id, encode_ctx_unsigned},
    reader::Reader,
    tag::Tag,
    writer::Writer,
};
use crate::services::value_codec::decode_application_data_value;
use crate::types::{DataValue, ObjectId, PropertyId};
use crate::{DecodeError, EncodeError};

pub const SERVICE_READ_PROPERTY: u8 = 0x0C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPropertyRequest {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub invoke_id: u8,
}

impl ReadPropertyRequest {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: true,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: self.invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY,
        }
        .encode(w)?;

        encode_ctx_object_id(w, 0, self.object_id.raw())?;
        encode_ctx_unsigned(w, 1, self.property_id.to_u32())?;
        if let Some(idx) = self.array_index {
            encode_ctx_unsigned(w, 2, idx)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadPropertyAck<'a> {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub value: DataValue<'a>,
}

impl<'a> ReadPropertyAck<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let object_id = match Tag::decode(r)? {
            Tag::Context { tag_num: 0, len } => {
                ObjectId::from_raw(decode_unsigned(r, len as usize)?)
            }
            _ => return Err(DecodeError::InvalidTag),
        };

        let property_id = match Tag::decode(r)? {
            Tag::Context { tag_num: 1, len } => {
                PropertyId::from_u32(decode_unsigned(r, len as usize)?)
            }
            _ => return Err(DecodeError::InvalidTag),
        };

        let next = Tag::decode(r)?;
        let (array_index, value_start_tag) = match next {
            Tag::Context { tag_num: 2, len } => {
                let idx = decode_unsigned(r, len as usize)?;
                (Some(idx), Tag::decode(r)?)
            }
            other => (None, other),
        };

        if value_start_tag != (Tag::Opening { tag_num: 3 }) {
            return Err(DecodeError::InvalidTag);
        }

        let value = decode_application_data_value(r)?;

        match Tag::decode(r)? {
            Tag::Closing { tag_num: 3 } => {}
            _ => return Err(DecodeError::InvalidTag),
        }

        Ok(Self {
            object_id,
            property_id,
            array_index,
            value,
        })
    }
}
