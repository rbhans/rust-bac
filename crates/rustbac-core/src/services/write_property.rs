use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{
        encode_closing_tag, encode_ctx_object_id, encode_ctx_unsigned, encode_opening_tag,
    },
    writer::Writer,
};
use crate::services::value_codec::encode_application_data_value;
use crate::types::{DataValue, ObjectId, PropertyId};
use crate::EncodeError;

pub const SERVICE_WRITE_PROPERTY: u8 = 0x0F;

#[derive(Debug, Clone, PartialEq)]
pub struct WritePropertyRequest<'a> {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub value: DataValue<'a>,
    pub array_index: Option<u32>,
    pub priority: Option<u8>,
    pub invoke_id: u8,
}

impl<'a> Default for WritePropertyRequest<'a> {
    fn default() -> Self {
        Self {
            object_id: ObjectId::new(crate::types::ObjectType::AnalogValue, 0),
            property_id: PropertyId::PresentValue,
            value: DataValue::Null,
            array_index: None,
            priority: None,
            invoke_id: 1,
        }
    }
}

impl<'a> WritePropertyRequest<'a> {
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
            service_choice: SERVICE_WRITE_PROPERTY,
        }
        .encode(w)?;

        encode_ctx_object_id(w, 0, self.object_id.raw())?;
        encode_ctx_unsigned(w, 1, self.property_id.to_u32())?;
        if let Some(idx) = self.array_index {
            encode_ctx_unsigned(w, 2, idx)?;
        }

        encode_opening_tag(w, 3)?;
        encode_application_data_value(w, &self.value)?;
        encode_closing_tag(w, 3)?;

        if let Some(priority) = self.priority {
            encode_ctx_unsigned(w, 4, priority as u32)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{WritePropertyRequest, SERVICE_WRITE_PROPERTY};
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{DataValue, ObjectId, ObjectType, PropertyId};

    #[test]
    fn encode_write_property_with_character_string() {
        let req = WritePropertyRequest {
            object_id: ObjectId::new(ObjectType::AnalogValue, 3),
            property_id: PropertyId::Description,
            value: DataValue::CharacterString("loop tuning pending"),
            priority: None,
            ..Default::default()
        };

        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_WRITE_PROPERTY);
    }
}
