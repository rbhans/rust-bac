use crate::apdu::ConfirmedRequestHeader;
use crate::EncodeError;

#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader, tag::Tag};
#[cfg(feature = "alloc")]
use crate::types::{BitString, ObjectId};
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_GET_ALARM_SUMMARY: u8 = 0x03;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetAlarmSummaryRequest {
    pub invoke_id: u8,
}

impl GetAlarmSummaryRequest {
    pub fn encode(&self, w: &mut crate::encoding::writer::Writer<'_>) -> Result<(), EncodeError> {
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: true,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: self.invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_ALARM_SUMMARY,
        }
        .encode(w)
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmSummaryItem<'a> {
    pub object_id: ObjectId,
    pub alarm_state: u32,
    pub acknowledged_transitions: BitString<'a>,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetAlarmSummaryAck<'a> {
    pub summaries: Vec<AlarmSummaryItem<'a>>,
}

#[cfg(feature = "alloc")]
impl<'a> GetAlarmSummaryAck<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let mut summaries = Vec::new();
        while !r.is_empty() {
            let object_id = match Tag::decode(r)? {
                Tag::Context { tag_num: 0, len } => {
                    if len != 4 {
                        return Err(DecodeError::InvalidLength);
                    }
                    ObjectId::from_raw(decode_unsigned(r, len as usize)?)
                }
                _ => return Err(DecodeError::InvalidTag),
            };

            let alarm_state = match Tag::decode(r)? {
                Tag::Context { tag_num: 1, len } => decode_unsigned(r, len as usize)?,
                _ => return Err(DecodeError::InvalidTag),
            };

            let acknowledged_transitions = match Tag::decode(r)? {
                Tag::Context { tag_num: 2, len } => {
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

            summaries.push(AlarmSummaryItem {
                object_id,
                alarm_state,
                acknowledged_transitions,
            });
        }
        Ok(Self { summaries })
    }
}

#[cfg(test)]
mod tests {
    use super::{GetAlarmSummaryRequest, SERVICE_GET_ALARM_SUMMARY};
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn encode_get_alarm_summary_request() {
        let req = GetAlarmSummaryRequest { invoke_id: 9 };
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_GET_ALARM_SUMMARY);
        assert_eq!(hdr.invoke_id, 9);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_get_alarm_summary_ack() {
        use super::GetAlarmSummaryAck;
        use crate::apdu::ComplexAckHeader;
        use crate::encoding::primitives::{encode_ctx_object_id, encode_ctx_unsigned};
        use crate::encoding::tag::Tag;
        use crate::types::{ObjectId, ObjectType};

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 9,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_ALARM_SUMMARY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogInput, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 1).unwrap();
        Tag::Context { tag_num: 2, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack_hdr = ComplexAckHeader::decode(&mut r).unwrap();
        let ack = GetAlarmSummaryAck::decode_after_header(&mut r).unwrap();
        assert_eq!(ack.summaries.len(), 1);
        assert_eq!(
            ack.summaries[0].object_id,
            ObjectId::new(ObjectType::AnalogInput, 1)
        );
        assert_eq!(ack.summaries[0].alarm_state, 1);
        assert_eq!(ack.summaries[0].acknowledged_transitions.unused_bits, 5);
    }
}
