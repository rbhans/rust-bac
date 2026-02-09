#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader, tag::Tag};
#[cfg(feature = "alloc")]
use crate::services::value_codec::decode_application_data_value_from_tag;
#[cfg(feature = "alloc")]
use crate::types::{DataValue, ObjectId, PropertyId};
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_CONFIRMED_COV_NOTIFICATION: u8 = 0x01;
pub const SERVICE_UNCONFIRMED_COV_NOTIFICATION: u8 = 0x02;

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct CovPropertyValue<'a> {
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub value: DataValue<'a>,
    pub priority: Option<u8>,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct CovNotificationRequest<'a> {
    pub subscriber_process_id: u32,
    pub initiating_device_id: ObjectId,
    pub monitored_object_id: ObjectId,
    pub time_remaining_seconds: u32,
    pub values: Vec<CovPropertyValue<'a>>,
}

#[cfg(feature = "alloc")]
impl<'a> CovNotificationRequest<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let subscriber_process_id = decode_required_ctx_unsigned(r, 0)?;
        let initiating_device_id = decode_required_ctx_object_id(r, 1)?;
        let monitored_object_id = decode_required_ctx_object_id(r, 2)?;
        let time_remaining_seconds = decode_required_ctx_unsigned(r, 3)?;

        match Tag::decode(r)? {
            Tag::Opening { tag_num: 4 } => {}
            _ => return Err(DecodeError::InvalidTag),
        }

        let mut values = Vec::new();
        loop {
            let property_start = Tag::decode(r)?;
            if property_start == (Tag::Closing { tag_num: 4 }) {
                break;
            }

            let property_id = match property_start {
                Tag::Context { tag_num: 0, len } => {
                    PropertyId::from_u32(decode_unsigned(r, len as usize)?)
                }
                _ => return Err(DecodeError::InvalidTag),
            };

            let next = Tag::decode(r)?;
            let (array_index, value_open_tag) = match next {
                Tag::Context { tag_num: 1, len } => {
                    let idx = decode_unsigned(r, len as usize)?;
                    (Some(idx), Tag::decode(r)?)
                }
                other => (None, other),
            };
            if value_open_tag != (Tag::Opening { tag_num: 2 }) {
                return Err(DecodeError::InvalidTag);
            }

            let value_tag = Tag::decode(r)?;
            let value = decode_application_data_value_from_tag(r, value_tag)?;
            match Tag::decode(r)? {
                Tag::Closing { tag_num: 2 } => {}
                _ => return Err(DecodeError::InvalidTag),
            }

            let checkpoint = *r;
            let priority = match Tag::decode(r)? {
                Tag::Context { tag_num: 3, len } => {
                    let p = decode_unsigned(r, len as usize)?;
                    if p > u8::MAX as u32 {
                        return Err(DecodeError::InvalidValue);
                    }
                    Some(p as u8)
                }
                _ => {
                    *r = checkpoint;
                    None
                }
            };

            values.push(CovPropertyValue {
                property_id,
                array_index,
                value,
                priority,
            });
        }

        Ok(Self {
            subscriber_process_id,
            initiating_device_id,
            monitored_object_id,
            time_remaining_seconds,
            values,
        })
    }
}

#[cfg(feature = "alloc")]
fn decode_required_ctx_unsigned(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context { tag_num, len } if tag_num == expected_tag_num => {
            decode_unsigned(r, len as usize)
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_required_ctx_object_id(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<ObjectId, DecodeError> {
    Ok(ObjectId::from_raw(decode_required_ctx_unsigned(
        r,
        expected_tag_num,
    )?))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "alloc")]
    use super::{CovNotificationRequest, SERVICE_UNCONFIRMED_COV_NOTIFICATION};
    #[cfg(feature = "alloc")]
    use crate::apdu::UnconfirmedRequestHeader;
    #[cfg(feature = "alloc")]
    use crate::encoding::{
        primitives::{encode_app_real, encode_ctx_unsigned},
        tag::Tag,
        writer::Writer,
    };
    #[cfg(feature = "alloc")]
    use crate::types::{ObjectId, ObjectType, PropertyId};

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_cov_notification_after_header() {
        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        UnconfirmedRequestHeader {
            service_choice: SERVICE_UNCONFIRMED_COV_NOTIFICATION,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, 77).unwrap();
        encode_ctx_unsigned(&mut w, 1, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 2, ObjectId::new(ObjectType::AnalogInput, 2).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 3, 120).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 42.25).unwrap();
        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 3, 8).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        let encoded = w.as_written();
        let mut r = crate::encoding::reader::Reader::new(encoded);
        let _header = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        let cov = CovNotificationRequest::decode_after_header(&mut r).unwrap();
        assert_eq!(cov.subscriber_process_id, 77);
        assert_eq!(
            cov.initiating_device_id,
            ObjectId::new(ObjectType::Device, 1)
        );
        assert_eq!(
            cov.monitored_object_id,
            ObjectId::new(ObjectType::AnalogInput, 2)
        );
        assert_eq!(cov.values.len(), 1);
        assert_eq!(cov.values[0].property_id, PropertyId::PresentValue);
        assert_eq!(cov.values[0].priority, Some(8));
    }
}
