use crate::apdu::ConfirmedRequestHeader;
use crate::EncodeError;

#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader, tag::Tag};
#[cfg(feature = "alloc")]
use crate::types::ObjectId;
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_GET_ENROLLMENT_SUMMARY: u8 = 0x04;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetEnrollmentSummaryRequest {
    pub invoke_id: u8,
}

impl GetEnrollmentSummaryRequest {
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
            service_choice: SERVICE_GET_ENROLLMENT_SUMMARY,
        }
        .encode(w)
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrollmentSummaryItem {
    pub object_id: ObjectId,
    pub event_type: u32,
    pub event_state: u32,
    pub priority: u32,
    pub notification_class: u32,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEnrollmentSummaryAck {
    pub summaries: Vec<EnrollmentSummaryItem>,
}

#[cfg(feature = "alloc")]
impl GetEnrollmentSummaryAck {
    pub fn decode_after_header(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
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

            let event_type = match Tag::decode(r)? {
                Tag::Context { tag_num: 1, len } => decode_unsigned(r, len as usize)?,
                _ => return Err(DecodeError::InvalidTag),
            };

            let event_state = match Tag::decode(r)? {
                Tag::Context { tag_num: 2, len } => decode_unsigned(r, len as usize)?,
                _ => return Err(DecodeError::InvalidTag),
            };

            let priority = match Tag::decode(r)? {
                Tag::Context { tag_num: 3, len } => decode_unsigned(r, len as usize)?,
                _ => return Err(DecodeError::InvalidTag),
            };

            let notification_class = match Tag::decode(r)? {
                Tag::Context { tag_num: 4, len } => decode_unsigned(r, len as usize)?,
                _ => return Err(DecodeError::InvalidTag),
            };

            summaries.push(EnrollmentSummaryItem {
                object_id,
                event_type,
                event_state,
                priority,
                notification_class,
            });
        }
        Ok(Self { summaries })
    }
}

#[cfg(test)]
mod tests {
    use super::{GetEnrollmentSummaryRequest, SERVICE_GET_ENROLLMENT_SUMMARY};
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn encode_get_enrollment_summary_request() {
        let req = GetEnrollmentSummaryRequest { invoke_id: 7 };
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_GET_ENROLLMENT_SUMMARY);
        assert_eq!(hdr.invoke_id, 7);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_get_enrollment_summary_ack() {
        use super::GetEnrollmentSummaryAck;
        use crate::apdu::ComplexAckHeader;
        use crate::encoding::primitives::{encode_ctx_object_id, encode_ctx_unsigned};
        use crate::types::{ObjectId, ObjectType};

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 7,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_ENROLLMENT_SUMMARY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogInput, 3).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 1).unwrap();
        encode_ctx_unsigned(&mut w, 2, 2).unwrap();
        encode_ctx_unsigned(&mut w, 3, 200).unwrap();
        encode_ctx_unsigned(&mut w, 4, 10).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack_hdr = ComplexAckHeader::decode(&mut r).unwrap();
        let ack = GetEnrollmentSummaryAck::decode_after_header(&mut r).unwrap();
        assert_eq!(ack.summaries.len(), 1);
        assert_eq!(ack.summaries[0].event_type, 1);
        assert_eq!(ack.summaries[0].event_state, 2);
        assert_eq!(ack.summaries[0].priority, 200);
        assert_eq!(ack.summaries[0].notification_class, 10);
    }
}
