use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_object_id, encode_ctx_unsigned},
    tag::Tag,
    writer::Writer,
};
use crate::services::value_codec::encode_application_data_value;
use crate::types::{DataValue, ObjectId, PropertyId};
use crate::EncodeError;

pub const SERVICE_WRITE_PROPERTY_MULTIPLE: u8 = 0x10;

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyWriteSpec<'a> {
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub value: DataValue<'a>,
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WriteAccessSpecification<'a> {
    pub object_id: ObjectId,
    pub properties: &'a [PropertyWriteSpec<'a>],
}

#[derive(Debug, Clone, PartialEq)]
pub struct WritePropertyMultipleRequest<'a> {
    pub specs: &'a [WriteAccessSpecification<'a>],
    pub invoke_id: u8,
}

impl<'a> WritePropertyMultipleRequest<'a> {
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
            service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
        }
        .encode(w)?;

        for spec in self.specs {
            encode_ctx_object_id(w, 0, spec.object_id.raw())?;
            Tag::Opening { tag_num: 1 }.encode(w)?;

            for prop in spec.properties {
                encode_ctx_unsigned(w, 0, prop.property_id.to_u32())?;
                if let Some(idx) = prop.array_index {
                    encode_ctx_unsigned(w, 1, idx)?;
                }

                Tag::Opening { tag_num: 2 }.encode(w)?;
                encode_application_data_value(w, &prop.value)?;
                Tag::Closing { tag_num: 2 }.encode(w)?;

                if let Some(priority) = prop.priority {
                    encode_ctx_unsigned(w, 3, priority as u32)?;
                }
            }

            Tag::Closing { tag_num: 1 }.encode(w)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PropertyWriteSpec, WriteAccessSpecification, WritePropertyMultipleRequest,
        SERVICE_WRITE_PROPERTY_MULTIPLE,
    };
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{DataValue, ObjectId, ObjectType, PropertyId};

    #[test]
    fn encode_write_property_multiple_request() {
        let writes = [
            PropertyWriteSpec {
                property_id: PropertyId::PresentValue,
                array_index: None,
                value: DataValue::Real(72.5),
                priority: Some(8),
            },
            PropertyWriteSpec {
                property_id: PropertyId::Description,
                array_index: None,
                value: DataValue::CharacterString("staged by rustbac"),
                priority: None,
            },
        ];

        let specs = [WriteAccessSpecification {
            object_id: ObjectId::new(ObjectType::AnalogOutput, 1),
            properties: &writes,
        }];

        let req = WritePropertyMultipleRequest {
            specs: &specs,
            invoke_id: 5,
        };

        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.invoke_id, 5);
        assert_eq!(header.service_choice, SERVICE_WRITE_PROPERTY_MULTIPLE);
        assert!(!r.is_empty());
    }
}
