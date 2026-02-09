use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{primitives::encode_ctx_object_id, writer::Writer};
use crate::types::ObjectId;
use crate::EncodeError;

#[cfg(feature = "alloc")]
use crate::encoding::{
    primitives::decode_unsigned,
    reader::Reader,
    tag::{AppTag, Tag},
};
#[cfg(feature = "alloc")]
use crate::types::BitString;
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_GET_EVENT_INFORMATION: u8 = 0x1D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetEventInformationRequest {
    pub last_received_object_id: Option<ObjectId>,
    pub invoke_id: u8,
}

impl GetEventInformationRequest {
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
            service_choice: SERVICE_GET_EVENT_INFORMATION,
        }
        .encode(w)?;

        if let Some(object_id) = self.last_received_object_id {
            encode_ctx_object_id(w, 0, object_id.raw())?;
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSummaryItem<'a> {
    pub object_id: ObjectId,
    pub event_state: u32,
    pub acknowledged_transitions: BitString<'a>,
    pub notify_type: u32,
    pub event_enable: BitString<'a>,
    pub event_priorities: [u32; 3],
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEventInformationAck<'a> {
    pub summaries: Vec<EventSummaryItem<'a>>,
    pub more_events: bool,
}

#[cfg(feature = "alloc")]
impl<'a> GetEventInformationAck<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        if Tag::decode(r)? != (Tag::Opening { tag_num: 0 }) {
            return Err(DecodeError::InvalidTag);
        }

        let mut summaries = Vec::new();
        loop {
            let tag = Tag::decode(r)?;
            if tag == (Tag::Closing { tag_num: 0 }) {
                break;
            }
            let object_id = decode_object_id_from_tag(r, tag)?;
            let event_state = decode_expected_context_unsigned(r, 1)?;
            let acknowledged_transitions = decode_expected_context_bit_string(r, 2)?;

            match Tag::decode(r)? {
                Tag::Opening { tag_num: 3 } => skip_constructed(r, 3)?,
                _ => return Err(DecodeError::InvalidTag),
            }

            let notify_type = decode_expected_context_unsigned(r, 4)?;
            let event_enable = decode_expected_context_bit_string(r, 5)?;

            match Tag::decode(r)? {
                Tag::Opening { tag_num: 6 } => {}
                _ => return Err(DecodeError::InvalidTag),
            }
            let mut priorities = [0u32; 3];
            for slot in &mut priorities {
                let priority_tag = Tag::decode(r)?;
                *slot = decode_unsigned_from_tag(r, priority_tag)?;
            }
            if Tag::decode(r)? != (Tag::Closing { tag_num: 6 }) {
                return Err(DecodeError::InvalidTag);
            }

            summaries.push(EventSummaryItem {
                object_id,
                event_state,
                acknowledged_transitions,
                notify_type,
                event_enable,
                event_priorities: priorities,
            });
        }

        let more_events = match Tag::decode(r)? {
            Tag::Context { tag_num: 1, len: 0 } => true,
            Tag::Context { tag_num: 1, len } => decode_unsigned(r, len as usize)? != 0,
            _ => return Err(DecodeError::InvalidTag),
        };

        Ok(Self {
            summaries,
            more_events,
        })
    }
}

#[cfg(feature = "alloc")]
fn decode_object_id_from_tag(r: &mut Reader<'_>, tag: Tag) -> Result<ObjectId, DecodeError> {
    match tag {
        Tag::Context { tag_num: 0, len } => {
            if len != 4 {
                return Err(DecodeError::InvalidLength);
            }
            Ok(ObjectId::from_raw(decode_unsigned(r, len as usize)?))
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_expected_context_unsigned(r: &mut Reader<'_>, tag_num: u8) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context {
            tag_num: found,
            len,
        } if found == tag_num => decode_unsigned(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_unsigned_from_tag(r: &mut Reader<'_>, tag: Tag) -> Result<u32, DecodeError> {
    match tag {
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len,
        } => decode_unsigned(r, len as usize),
        Tag::Context { len, .. } => decode_unsigned(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_expected_context_bit_string<'a>(
    r: &mut Reader<'a>,
    tag_num: u8,
) -> Result<BitString<'a>, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context {
            tag_num: found,
            len,
        } if found == tag_num => {
            if len == 0 {
                return Err(DecodeError::InvalidLength);
            }
            let raw = r.read_exact(len as usize)?;
            if raw[0] > 7 {
                return Err(DecodeError::InvalidValue);
            }
            Ok(BitString {
                unused_bits: raw[0],
                data: &raw[1..],
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
    use super::GetEventInformationAck;
    use super::{GetEventInformationRequest, SERVICE_GET_EVENT_INFORMATION};
    #[cfg(feature = "alloc")]
    use crate::apdu::ComplexAckHeader;
    use crate::apdu::ConfirmedRequestHeader;
    #[cfg(feature = "alloc")]
    use crate::encoding::primitives::{encode_ctx_object_id, encode_ctx_unsigned};
    #[cfg(feature = "alloc")]
    use crate::encoding::tag::Tag;
    use crate::encoding::{reader::Reader, writer::Writer};
    #[cfg(feature = "alloc")]
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn encode_get_event_information_request() {
        let req = GetEventInformationRequest {
            last_received_object_id: None,
            invoke_id: 9,
        };
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_GET_EVENT_INFORMATION);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_get_event_information_ack() {
        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 9,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_EVENT_INFORMATION,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogInput, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 2).unwrap();
        Tag::Context { tag_num: 2, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 1, 42).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 4, 0).unwrap();
        Tag::Context { tag_num: 5, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();
        Tag::Opening { tag_num: 6 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        encode_ctx_unsigned(&mut w, 1, 2).unwrap();
        encode_ctx_unsigned(&mut w, 2, 3).unwrap();
        Tag::Closing { tag_num: 6 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Context { tag_num: 1, len: 1 }.encode(&mut w).unwrap();
        w.write_u8(0).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack_hdr = ComplexAckHeader::decode(&mut r).unwrap();
        let ack = GetEventInformationAck::decode_after_header(&mut r).unwrap();
        assert_eq!(ack.summaries.len(), 1);
        assert!(!ack.more_events);
    }
}
