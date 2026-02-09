use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_object_id, encode_ctx_unsigned},
    tag::Tag,
    writer::Writer,
};
use crate::types::ObjectId;
use crate::EncodeError;

pub const SERVICE_SUBSCRIBE_COV: u8 = 0x05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscribeCovRequest {
    pub subscriber_process_id: u32,
    pub monitored_object_id: ObjectId,
    pub issue_confirmed_notifications: Option<bool>,
    pub lifetime_seconds: Option<u32>,
    pub invoke_id: u8,
}

impl SubscribeCovRequest {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: self.invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_SUBSCRIBE_COV,
        }
        .encode(w)?;

        encode_ctx_unsigned(w, 0, self.subscriber_process_id)?;
        encode_ctx_object_id(w, 1, self.monitored_object_id.raw())?;
        if let Some(issue_confirmed) = self.issue_confirmed_notifications {
            Tag::Context { tag_num: 2, len: 1 }.encode(w)?;
            w.write_u8(if issue_confirmed { 1 } else { 0 })?;
        }
        if let Some(lifetime_seconds) = self.lifetime_seconds {
            encode_ctx_unsigned(w, 3, lifetime_seconds)?;
        }
        Ok(())
    }

    pub fn cancel(
        subscriber_process_id: u32,
        monitored_object_id: ObjectId,
        invoke_id: u8,
    ) -> Self {
        Self {
            subscriber_process_id,
            monitored_object_id,
            issue_confirmed_notifications: None,
            lifetime_seconds: None,
            invoke_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SubscribeCovRequest, SERVICE_SUBSCRIBE_COV};
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn encode_subscribe_cov_request() {
        let req = SubscribeCovRequest {
            subscriber_process_id: 7,
            monitored_object_id: ObjectId::new(ObjectType::AnalogInput, 2),
            issue_confirmed_notifications: Some(false),
            lifetime_seconds: Some(600),
            invoke_id: 3,
        };

        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.invoke_id, 3);
        assert_eq!(header.service_choice, SERVICE_SUBSCRIBE_COV);
        assert!(!r.is_empty());
    }
}
