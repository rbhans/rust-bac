use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{decode_unsigned, encode_ctx_object_id, encode_ctx_unsigned},
    reader::Reader,
    tag::Tag,
    writer::Writer,
};
#[cfg(not(feature = "alloc"))]
use crate::services::value_codec::decode_application_data_value;
#[cfg(feature = "alloc")]
use crate::services::value_codec::decode_application_data_value_from_tag;
use crate::types::{DataValue, ObjectId, PropertyId};
use crate::{DecodeError, EncodeError};

#[cfg(feature = "alloc")]
extern crate alloc;

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

        let value = decode_property_value(r)?;

        Ok(Self {
            object_id,
            property_id,
            array_index,
            value,
        })
    }
}

/// Decode the `[3] propertyValue` field of a ReadProperty ACK and consume the
/// matching `Tag::Closing { tag_num: 3 }`.
///
/// Array-valued properties (`object-list`, `priority-array`, etc.) encode
/// multiple elements directly between the opening and closing tags. A single
/// inner element returns its bare `DataValue`; multiple elements return
/// `DataValue::Constructed { tag_num: 3, values }`; an empty array returns
/// `DataValue::Null`. This mirrors what callers already expect (see
/// `walk.rs::extract_object_ids`).
#[cfg(feature = "alloc")]
fn decode_property_value<'a>(r: &mut Reader<'a>) -> Result<DataValue<'a>, DecodeError> {
    let mut values: alloc::vec::Vec<DataValue<'a>> = alloc::vec::Vec::new();
    loop {
        let next = Tag::decode(r)?;
        if next == (Tag::Closing { tag_num: 3 }) {
            return Ok(match values.len() {
                0 => DataValue::Null,
                1 => values.into_iter().next().unwrap(),
                _ => DataValue::Constructed { tag_num: 3, values },
            });
        }
        values.push(decode_application_data_value_from_tag(r, next)?);
    }
}

#[cfg(not(feature = "alloc"))]
fn decode_property_value<'a>(r: &mut Reader<'a>) -> Result<DataValue<'a>, DecodeError> {
    let value = decode_application_data_value(r)?;
    match Tag::decode(r)? {
        Tag::Closing { tag_num: 3 } => Ok(value),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::*;
    use crate::encoding::{
        primitives::{encode_closing_tag, encode_opening_tag},
        tag::AppTag,
    };
    use crate::types::ObjectType;
    use alloc::vec;

    fn write_ack_header(w: &mut Writer<'_>, object_id: ObjectId, property_id: PropertyId) {
        encode_ctx_object_id(w, 0, object_id.raw()).unwrap();
        encode_ctx_unsigned(w, 1, property_id.to_u32()).unwrap();
    }

    #[test]
    fn decode_single_primitive_value_preserves_shape() {
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        let oid = ObjectId::new(ObjectType::AnalogOutput, 0);
        write_ack_header(&mut w, oid, PropertyId::PresentValue);
        encode_opening_tag(&mut w, 3).unwrap();
        Tag::Application {
            tag: AppTag::Real,
            len: 4,
        }
        .encode(&mut w)
        .unwrap();
        w.write_all(&75.0_f32.to_bits().to_be_bytes()).unwrap();
        encode_closing_tag(&mut w, 3).unwrap();

        let mut r = Reader::new(w.as_written());
        let ack = ReadPropertyAck::decode_after_header(&mut r).unwrap();
        assert_eq!(ack.value, DataValue::Real(75.0));
    }

    #[test]
    fn decode_object_list_returns_constructed() {
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        let device = ObjectId::new(ObjectType::Device, 1234);
        let ao = ObjectId::new(ObjectType::AnalogOutput, 0);
        write_ack_header(&mut w, device, PropertyId::ObjectList);
        encode_opening_tag(&mut w, 3).unwrap();
        for oid in &[device, ao] {
            Tag::Application {
                tag: AppTag::ObjectId,
                len: 4,
            }
            .encode(&mut w)
            .unwrap();
            w.write_all(&oid.raw().to_be_bytes()).unwrap();
        }
        encode_closing_tag(&mut w, 3).unwrap();

        let mut r = Reader::new(w.as_written());
        let ack = ReadPropertyAck::decode_after_header(&mut r).unwrap();
        assert_eq!(
            ack.value,
            DataValue::Constructed {
                tag_num: 3,
                values: vec![DataValue::ObjectId(device), DataValue::ObjectId(ao)],
            }
        );
    }

    #[test]
    fn decode_priority_array_returns_constructed_of_context_tagged() {
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        let oid = ObjectId::new(ObjectType::AnalogOutput, 0);
        write_ack_header(&mut w, oid, PropertyId::PriorityArray);
        encode_opening_tag(&mut w, 3).unwrap();
        // 3 Null slots, 1 Real slot (priority 8 = 75.0), 12 Null slots.
        for _ in 0..3 {
            Tag::Context { tag_num: 0, len: 0 }.encode(&mut w).unwrap();
        }
        Tag::Context { tag_num: 1, len: 4 }.encode(&mut w).unwrap();
        w.write_all(&75.0_f32.to_bits().to_be_bytes()).unwrap();
        for _ in 0..12 {
            Tag::Context { tag_num: 0, len: 0 }.encode(&mut w).unwrap();
        }
        encode_closing_tag(&mut w, 3).unwrap();

        let mut r = Reader::new(w.as_written());
        let ack = ReadPropertyAck::decode_after_header(&mut r).unwrap();
        let values = match ack.value {
            DataValue::Constructed { tag_num: 3, values } => values,
            other => panic!("expected Constructed, got {other:?}"),
        };
        assert_eq!(values.len(), 16);
        assert!(matches!(
            values[3],
            DataValue::ContextTagged {
                tag_num: 1,
                data,
            } if data == 75.0_f32.to_bits().to_be_bytes()
        ));
        assert!(matches!(
            values[0],
            DataValue::ContextTagged {
                tag_num: 0,
                data: &[],
            }
        ));
    }

    #[test]
    fn decode_empty_value_field_yields_null() {
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        let oid = ObjectId::new(ObjectType::AnalogOutput, 0);
        write_ack_header(&mut w, oid, PropertyId::PresentValue);
        encode_opening_tag(&mut w, 3).unwrap();
        encode_closing_tag(&mut w, 3).unwrap();

        let mut r = Reader::new(w.as_written());
        let ack = ReadPropertyAck::decode_after_header(&mut r).unwrap();
        assert_eq!(ack.value, DataValue::Null);
    }
}
