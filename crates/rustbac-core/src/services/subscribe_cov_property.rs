use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_object_id, encode_ctx_unsigned},
    tag::Tag,
    writer::Writer,
};
use crate::types::{ObjectId, PropertyId};
use crate::EncodeError;

pub const SERVICE_SUBSCRIBE_COV_PROPERTY: u8 = 0x1C;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubscribeCovPropertyRequest {
    pub subscriber_process_id: u32,
    pub monitored_object_id: ObjectId,
    pub issue_confirmed_notifications: Option<bool>,
    pub lifetime_seconds: Option<u32>,
    pub monitored_property_id: PropertyId,
    pub monitored_property_array_index: Option<u32>,
    pub cov_increment: Option<f32>,
    pub invoke_id: u8,
}

impl SubscribeCovPropertyRequest {
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
            service_choice: SERVICE_SUBSCRIBE_COV_PROPERTY,
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

        Tag::Opening { tag_num: 4 }.encode(w)?;
        encode_ctx_unsigned(w, 0, self.monitored_property_id.to_u32())?;
        if let Some(array_index) = self.monitored_property_array_index {
            encode_ctx_unsigned(w, 1, array_index)?;
        }
        Tag::Closing { tag_num: 4 }.encode(w)?;

        if let Some(cov_increment) = self.cov_increment {
            Tag::Context { tag_num: 5, len: 4 }.encode(w)?;
            w.write_all(&cov_increment.to_bits().to_be_bytes())?;
        }

        Ok(())
    }

    pub fn cancel(
        subscriber_process_id: u32,
        monitored_object_id: ObjectId,
        monitored_property_id: PropertyId,
        monitored_property_array_index: Option<u32>,
        invoke_id: u8,
    ) -> Self {
        Self {
            subscriber_process_id,
            monitored_object_id,
            issue_confirmed_notifications: None,
            lifetime_seconds: None,
            monitored_property_id,
            monitored_property_array_index,
            cov_increment: None,
            invoke_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SubscribeCovPropertyRequest, SERVICE_SUBSCRIBE_COV_PROPERTY};
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{ObjectId, ObjectType, PropertyId};

    #[test]
    fn encode_subscribe_cov_property_request() {
        let req = SubscribeCovPropertyRequest {
            subscriber_process_id: 9,
            monitored_object_id: ObjectId::new(ObjectType::AnalogInput, 11),
            issue_confirmed_notifications: Some(true),
            lifetime_seconds: Some(300),
            monitored_property_id: PropertyId::PresentValue,
            monitored_property_array_index: None,
            cov_increment: Some(0.5),
            invoke_id: 4,
        };

        let mut buf = [0u8; 96];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.invoke_id, 4);
        assert_eq!(header.service_choice, SERVICE_SUBSCRIBE_COV_PROPERTY);
        assert!(!r.is_empty());
    }
}
