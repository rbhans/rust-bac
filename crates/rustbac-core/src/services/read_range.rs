use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{
        encode_app_signed, encode_app_unsigned, encode_ctx_object_id, encode_ctx_unsigned,
    },
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::types::{Date, ObjectId, PropertyId, Time};
use crate::EncodeError;

#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader};
#[cfg(feature = "alloc")]
use crate::services::value_codec::decode_application_data_value_from_tag;
#[cfg(feature = "alloc")]
use crate::types::{BitString, DataValue};
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_READ_RANGE: u8 = 0x1A;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadRangeSpecifier {
    ByPosition { reference_index: i32, count: i16 },
    BySequenceNumber { reference_sequence: u32, count: i16 },
    ByTime { date: Date, time: Time, count: i16 },
    ReadAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadRangeRequest {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub range: ReadRangeSpecifier,
    pub invoke_id: u8,
}

impl ReadRangeRequest {
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
            service_choice: SERVICE_READ_RANGE,
        }
        .encode(w)?;

        encode_ctx_object_id(w, 0, self.object_id.raw())?;
        encode_ctx_unsigned(w, 1, self.property_id.to_u32())?;
        if let Some(array_index) = self.array_index {
            encode_ctx_unsigned(w, 2, array_index)?;
        }

        match self.range {
            ReadRangeSpecifier::ByPosition {
                reference_index,
                count,
            } => {
                let reference_index =
                    u32::try_from(reference_index).map_err(|_| EncodeError::ValueOutOfRange)?;
                Tag::Opening { tag_num: 3 }.encode(w)?;
                encode_app_unsigned(w, reference_index)?;
                encode_app_signed(w, count as i32)?;
                Tag::Closing { tag_num: 3 }.encode(w)?;
            }
            ReadRangeSpecifier::BySequenceNumber {
                reference_sequence,
                count,
            } => {
                Tag::Opening { tag_num: 6 }.encode(w)?;
                encode_app_unsigned(w, reference_sequence)?;
                encode_app_signed(w, count as i32)?;
                Tag::Closing { tag_num: 6 }.encode(w)?;
            }
            ReadRangeSpecifier::ByTime { date, time, count } => {
                Tag::Opening { tag_num: 7 }.encode(w)?;
                Tag::Application {
                    tag: AppTag::Date,
                    len: 4,
                }
                .encode(w)?;
                w.write_all(&[date.year_since_1900, date.month, date.day, date.weekday])?;
                Tag::Application {
                    tag: AppTag::Time,
                    len: 4,
                }
                .encode(w)?;
                w.write_all(&[time.hour, time.minute, time.second, time.hundredths])?;
                encode_app_signed(w, count as i32)?;
                Tag::Closing { tag_num: 7 }.encode(w)?;
            }
            ReadRangeSpecifier::ReadAll => {}
        }

        Ok(())
    }

    pub fn by_position(
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        reference_index: i32,
        count: i16,
        invoke_id: u8,
    ) -> Self {
        Self {
            object_id,
            property_id,
            array_index,
            range: ReadRangeSpecifier::ByPosition {
                reference_index,
                count,
            },
            invoke_id,
        }
    }

    pub fn by_sequence_number(
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        reference_sequence: u32,
        count: i16,
        invoke_id: u8,
    ) -> Self {
        Self {
            object_id,
            property_id,
            array_index,
            range: ReadRangeSpecifier::BySequenceNumber {
                reference_sequence,
                count,
            },
            invoke_id,
        }
    }

    pub fn by_time(
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        date: Date,
        time: Time,
        count: i16,
        invoke_id: u8,
    ) -> Self {
        Self {
            object_id,
            property_id,
            array_index,
            range: ReadRangeSpecifier::ByTime { date, time, count },
            invoke_id,
        }
    }

    pub fn read_all(
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        invoke_id: u8,
    ) -> Self {
        Self {
            object_id,
            property_id,
            array_index,
            range: ReadRangeSpecifier::ReadAll,
            invoke_id,
        }
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct ReadRangeAck<'a> {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub result_flags: BitString<'a>,
    pub item_count: u32,
    pub items: Vec<DataValue<'a>>,
}

#[cfg(feature = "alloc")]
impl<'a> ReadRangeAck<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let object_id = match Tag::decode(r)? {
            Tag::Context { tag_num: 0, len } => {
                if len != 4 {
                    return Err(DecodeError::InvalidLength);
                }
                ObjectId::from_raw(r.read_be_u32()?)
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
        let (array_index, result_flags_tag) = match next {
            Tag::Context { tag_num: 2, len } => {
                let idx = decode_unsigned(r, len as usize)?;
                (Some(idx), Tag::decode(r)?)
            }
            other => (None, other),
        };

        let result_flags = match result_flags_tag {
            Tag::Context { tag_num: 3, len } => {
                if len == 0 {
                    return Err(DecodeError::InvalidLength);
                }
                let raw = r.read_exact(len as usize)?;
                if raw[0] > 7 {
                    return Err(DecodeError::InvalidValue);
                }
                BitString {
                    unused_bits: raw[0],
                    data: &raw[1..],
                }
            }
            _ => return Err(DecodeError::InvalidTag),
        };

        let item_count = match Tag::decode(r)? {
            Tag::Context { tag_num: 4, len } => decode_unsigned(r, len as usize)?,
            _ => return Err(DecodeError::InvalidTag),
        };

        match Tag::decode(r)? {
            Tag::Opening { tag_num: 5 } => {}
            _ => return Err(DecodeError::InvalidTag),
        }

        let mut items = Vec::new();
        loop {
            let tag = Tag::decode(r)?;
            if tag == (Tag::Closing { tag_num: 5 }) {
                break;
            }

            let value = match tag {
                Tag::Application { .. } => decode_application_data_value_from_tag(r, tag)?,
                Tag::Context { .. } | Tag::Opening { .. } | Tag::Closing { .. } => {
                    return Err(DecodeError::Unsupported);
                }
            };
            items.push(value);
        }

        Ok(Self {
            object_id,
            property_id,
            array_index,
            result_flags,
            item_count,
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "alloc")]
    use super::ReadRangeAck;
    use super::{ReadRangeRequest, ReadRangeSpecifier, SERVICE_READ_RANGE};
    #[cfg(feature = "alloc")]
    use crate::apdu::ComplexAckHeader;
    use crate::apdu::ConfirmedRequestHeader;
    #[cfg(feature = "alloc")]
    use crate::encoding::primitives::{encode_app_real, encode_ctx_object_id, encode_ctx_unsigned};
    #[cfg(feature = "alloc")]
    use crate::encoding::tag::Tag;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{ObjectId, ObjectType, PropertyId};

    #[test]
    fn encode_read_range_request_by_position() {
        let req = ReadRangeRequest {
            object_id: ObjectId::new(ObjectType::TrendLog, 1),
            property_id: PropertyId::PresentValue,
            array_index: None,
            range: ReadRangeSpecifier::ByPosition {
                reference_index: 1,
                count: 10,
            },
            invoke_id: 3,
        };

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.service_choice, SERVICE_READ_RANGE);
        assert_eq!(header.invoke_id, 3);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_read_range_ack_minimal() {
        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 9,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_RANGE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::TrendLog, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Context { tag_num: 3, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();
        encode_ctx_unsigned(&mut w, 4, 2).unwrap();
        Tag::Opening { tag_num: 5 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 10.0).unwrap();
        encode_app_real(&mut w, 11.0).unwrap();
        Tag::Closing { tag_num: 5 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack = ComplexAckHeader::decode(&mut r).unwrap();
        let parsed = ReadRangeAck::decode_after_header(&mut r).unwrap();
        assert_eq!(parsed.item_count, 2);
        assert_eq!(parsed.items.len(), 2);
    }
}
