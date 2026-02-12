#[cfg(feature = "alloc")]
use crate::encoding::{
    primitives::{decode_ctx_character_string, decode_unsigned},
    reader::Reader,
    tag::{AppTag, Tag},
};
#[cfg(feature = "alloc")]
use crate::services::acknowledge_alarm::TimeStamp;
#[cfg(feature = "alloc")]
use crate::services::{decode_required_ctx_object_id, decode_required_ctx_unsigned};
#[cfg(feature = "alloc")]
use crate::types::{Date, ObjectId, Time};
#[cfg(feature = "alloc")]
use crate::DecodeError;

pub const SERVICE_CONFIRMED_EVENT_NOTIFICATION: u8 = 0x02;
pub const SERVICE_UNCONFIRMED_EVENT_NOTIFICATION: u8 = 0x03;

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventNotificationRequest<'a> {
    pub process_id: u32,
    pub initiating_device_id: ObjectId,
    pub event_object_id: ObjectId,
    pub timestamp: TimeStamp,
    pub notification_class: u32,
    pub priority: u32,
    pub event_type: u32,
    pub message_text: Option<&'a str>,
    pub notify_type: u32,
    pub ack_required: Option<bool>,
    pub from_state: u32,
    pub to_state: u32,
}

#[cfg(feature = "alloc")]
impl<'a> EventNotificationRequest<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let process_id = decode_required_ctx_unsigned(r, 0)?;
        let initiating_device_id = decode_required_ctx_object_id(r, 1)?;
        let event_object_id = decode_required_ctx_object_id(r, 2)?;
        let timestamp = decode_required_ctx_timestamp(r, 3)?;
        let notification_class = decode_required_ctx_unsigned(r, 4)?;
        let priority = decode_required_ctx_unsigned(r, 5)?;
        let event_type = decode_required_ctx_unsigned(r, 6)?;

        let checkpoint = *r;
        let message_text = match Tag::decode(r)? {
            Tag::Context { tag_num: 7, len } => Some(decode_ctx_character_string(r, len as usize)?),
            _ => {
                *r = checkpoint;
                None
            }
        };

        let notify_type = decode_required_ctx_unsigned(r, 8)?;

        let checkpoint = *r;
        let ack_required = match Tag::decode(r)? {
            Tag::Context { tag_num: 9, len } => Some(len != 0),
            _ => {
                *r = checkpoint;
                None
            }
        };

        let from_state = decode_required_ctx_unsigned(r, 10)?;
        let to_state = decode_required_ctx_unsigned(r, 11)?;

        let checkpoint = *r;
        if Tag::decode(r)? == (Tag::Opening { tag_num: 12 }) {
            skip_constructed(r, 12)?;
        } else {
            *r = checkpoint;
        }

        Ok(Self {
            process_id,
            initiating_device_id,
            event_object_id,
            timestamp,
            notification_class,
            priority,
            event_type,
            message_text,
            notify_type,
            ack_required,
            from_state,
            to_state,
        })
    }
}

#[cfg(feature = "alloc")]
fn decode_required_ctx_timestamp(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<TimeStamp, DecodeError> {
    match Tag::decode(r)? {
        Tag::Opening { tag_num } if tag_num == expected_tag_num => {}
        _ => return Err(DecodeError::InvalidTag),
    }

    let timestamp = match Tag::decode(r)? {
        Tag::Context { tag_num: 0, len: 4 } => {
            let raw = r.read_exact(4)?;
            TimeStamp::Time(Time {
                hour: raw[0],
                minute: raw[1],
                second: raw[2],
                hundredths: raw[3],
            })
        }
        Tag::Context { tag_num: 1, len } => {
            TimeStamp::SequenceNumber(decode_unsigned(r, len as usize)?)
        }
        Tag::Opening { tag_num: 2 } => {
            let date = decode_app_date(r)?;
            let time = decode_app_time(r)?;
            if Tag::decode(r)? != (Tag::Closing { tag_num: 2 }) {
                return Err(DecodeError::InvalidTag);
            }
            TimeStamp::DateTime { date, time }
        }
        _ => return Err(DecodeError::InvalidTag),
    };

    match Tag::decode(r)? {
        Tag::Closing { tag_num } if tag_num == expected_tag_num => Ok(timestamp),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_app_date(r: &mut Reader<'_>) -> Result<Date, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::Date,
            len: 4,
        } => {
            let raw = r.read_exact(4)?;
            Ok(Date {
                year_since_1900: raw[0],
                month: raw[1],
                day: raw[2],
                weekday: raw[3],
            })
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_app_time(r: &mut Reader<'_>) -> Result<Time, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::Time,
            len: 4,
        } => {
            let raw = r.read_exact(4)?;
            Ok(Time {
                hour: raw[0],
                minute: raw[1],
                second: raw[2],
                hundredths: raw[3],
            })
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn skip_constructed(r: &mut Reader<'_>, tag_num: u8) -> Result<(), DecodeError> {
    loop {
        let tag = Tag::decode(r)?;
        match tag {
            Tag::Closing { tag_num: closing } if closing == tag_num => return Ok(()),
            Tag::Opening { tag_num: nested } => skip_constructed(r, nested)?,
            Tag::Application { len, .. } | Tag::Context { len, .. } => {
                r.read_exact(len as usize)?;
            }
            Tag::Closing { .. } => return Err(DecodeError::InvalidTag),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "alloc")]
    use super::{EventNotificationRequest, SERVICE_UNCONFIRMED_EVENT_NOTIFICATION};
    #[cfg(feature = "alloc")]
    use crate::apdu::UnconfirmedRequestHeader;
    #[cfg(feature = "alloc")]
    use crate::encoding::{
        primitives::{encode_ctx_character_string, encode_ctx_object_id, encode_ctx_unsigned},
        reader::Reader,
        tag::Tag,
        writer::Writer,
    };
    #[cfg(feature = "alloc")]
    use crate::services::acknowledge_alarm::TimeStamp;
    #[cfg(feature = "alloc")]
    use crate::types::{ObjectId, ObjectType};

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_event_notification_after_header() {
        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        UnconfirmedRequestHeader {
            service_choice: SERVICE_UNCONFIRMED_EVENT_NOTIFICATION,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, 19).unwrap();
        encode_ctx_object_id(&mut w, 1, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        encode_ctx_object_id(&mut w, 2, ObjectId::new(ObjectType::AnalogInput, 3).raw()).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 1, 42).unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 4, 7).unwrap();
        encode_ctx_unsigned(&mut w, 5, 100).unwrap();
        encode_ctx_unsigned(&mut w, 6, 2).unwrap();
        encode_ctx_character_string(&mut w, 7, "alarm message").unwrap();
        encode_ctx_unsigned(&mut w, 8, 0).unwrap();
        Tag::Context { tag_num: 9, len: 1 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 10, 2).unwrap();
        encode_ctx_unsigned(&mut w, 11, 0).unwrap();
        Tag::Opening { tag_num: 12 }.encode(&mut w).unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 12 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _header = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        let notification = EventNotificationRequest::decode_after_header(&mut r).unwrap();
        assert_eq!(notification.process_id, 19);
        assert_eq!(
            notification.initiating_device_id,
            ObjectId::new(ObjectType::Device, 1)
        );
        assert_eq!(
            notification.event_object_id,
            ObjectId::new(ObjectType::AnalogInput, 3)
        );
        assert_eq!(notification.timestamp, TimeStamp::SequenceNumber(42));
        assert_eq!(notification.message_text, Some("alarm message"));
        assert_eq!(notification.ack_required, Some(true));
        assert_eq!(notification.from_state, 2);
        assert_eq!(notification.to_state, 0);
    }
}
