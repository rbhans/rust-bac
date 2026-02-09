use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_object_id, encode_ctx_unsigned},
    tag::Tag,
    writer::Writer,
};
use crate::services::value_codec::encode_application_data_value;
use crate::types::{DataValue, ObjectId, PropertyId};
use crate::EncodeError;

pub const SERVICE_ADD_LIST_ELEMENT: u8 = 0x08;
pub const SERVICE_REMOVE_LIST_ELEMENT: u8 = 0x09;

#[derive(Debug, Clone, PartialEq)]
pub struct AddListElementRequest<'a> {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub elements: &'a [DataValue<'a>],
    pub invoke_id: u8,
}

impl<'a> AddListElementRequest<'a> {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        encode_list_element_request(w, self, SERVICE_ADD_LIST_ELEMENT)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoveListElementRequest<'a> {
    pub object_id: ObjectId,
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub elements: &'a [DataValue<'a>],
    pub invoke_id: u8,
}

impl<'a> RemoveListElementRequest<'a> {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        encode_list_element_request(
            w,
            &AddListElementRequest {
                object_id: self.object_id,
                property_id: self.property_id,
                array_index: self.array_index,
                elements: self.elements,
                invoke_id: self.invoke_id,
            },
            SERVICE_REMOVE_LIST_ELEMENT,
        )
    }
}

fn encode_list_element_request(
    w: &mut Writer<'_>,
    req: &AddListElementRequest<'_>,
    service_choice: u8,
) -> Result<(), EncodeError> {
    ConfirmedRequestHeader {
        segmented: false,
        more_follows: false,
        segmented_response_accepted: false,
        max_segments: 0,
        max_apdu: 5,
        invoke_id: req.invoke_id,
        sequence_number: None,
        proposed_window_size: None,
        service_choice,
    }
    .encode(w)?;
    encode_ctx_object_id(w, 0, req.object_id.raw())?;
    encode_ctx_unsigned(w, 1, req.property_id.to_u32())?;
    if let Some(array_index) = req.array_index {
        encode_ctx_unsigned(w, 2, array_index)?;
    }
    Tag::Opening { tag_num: 3 }.encode(w)?;
    for value in req.elements {
        encode_application_data_value(w, value)?;
    }
    Tag::Closing { tag_num: 3 }.encode(w)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AddListElementRequest, RemoveListElementRequest, SERVICE_ADD_LIST_ELEMENT,
        SERVICE_REMOVE_LIST_ELEMENT,
    };
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{DataValue, ObjectId, ObjectType, PropertyId};

    #[test]
    fn encode_add_list_element_request() {
        let values = [DataValue::Unsigned(1), DataValue::Unsigned(2)];
        let req = AddListElementRequest {
            object_id: ObjectId::new(ObjectType::AnalogValue, 1),
            property_id: PropertyId::Proprietary(512),
            array_index: None,
            elements: &values,
            invoke_id: 7,
        };
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_ADD_LIST_ELEMENT);
    }

    #[test]
    fn encode_remove_list_element_request() {
        let values = [DataValue::Enumerated(3)];
        let req = RemoveListElementRequest {
            object_id: ObjectId::new(ObjectType::TrendLog, 1),
            property_id: PropertyId::Proprietary(513),
            array_index: None,
            elements: &values,
            invoke_id: 8,
        };
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_REMOVE_LIST_ELEMENT);
    }
}
