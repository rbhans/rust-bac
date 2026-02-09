use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{decode_unsigned, encode_ctx_object_id, encode_ctx_unsigned},
    reader::Reader,
    tag::Tag,
    writer::Writer,
};
use crate::types::{ObjectId, ObjectType};
use crate::{DecodeError, EncodeError};

pub const SERVICE_CREATE_OBJECT: u8 = 0x0A;
pub const SERVICE_DELETE_OBJECT: u8 = 0x0B;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateObjectSpecifier {
    ObjectType(ObjectType),
    ObjectId(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateObjectRequest {
    pub specifier: CreateObjectSpecifier,
    pub invoke_id: u8,
}

impl CreateObjectRequest {
    pub fn by_type(object_type: ObjectType, invoke_id: u8) -> Self {
        Self {
            specifier: CreateObjectSpecifier::ObjectType(object_type),
            invoke_id,
        }
    }

    pub fn by_id(object_id: ObjectId, invoke_id: u8) -> Self {
        Self {
            specifier: CreateObjectSpecifier::ObjectId(object_id),
            invoke_id,
        }
    }

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
            service_choice: SERVICE_CREATE_OBJECT,
        }
        .encode(w)?;

        match self.specifier {
            CreateObjectSpecifier::ObjectType(object_type) => {
                encode_ctx_unsigned(w, 0, object_type.to_u16() as u32)?
            }
            CreateObjectSpecifier::ObjectId(object_id) => {
                encode_ctx_object_id(w, 1, object_id.raw())?
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateObjectAck {
    pub object_id: ObjectId,
}

impl CreateObjectAck {
    pub fn decode_after_header(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let object_id = match Tag::decode(r)? {
            Tag::Context { tag_num: 0, len } => {
                if len != 4 {
                    return Err(DecodeError::InvalidLength);
                }
                ObjectId::from_raw(decode_unsigned(r, len as usize)?)
            }
            _ => return Err(DecodeError::InvalidTag),
        };
        Ok(Self { object_id })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteObjectRequest {
    pub object_id: ObjectId,
    pub invoke_id: u8,
}

impl DeleteObjectRequest {
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
            service_choice: SERVICE_DELETE_OBJECT,
        }
        .encode(w)?;
        encode_ctx_object_id(w, 0, self.object_id.raw())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateObjectAck, CreateObjectRequest, DeleteObjectRequest, SERVICE_CREATE_OBJECT,
        SERVICE_DELETE_OBJECT,
    };
    use crate::apdu::{ComplexAckHeader, ConfirmedRequestHeader};
    use crate::encoding::primitives::encode_ctx_object_id;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn encode_create_object_request() {
        let req = CreateObjectRequest::by_type(ObjectType::AnalogValue, 3);
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_CREATE_OBJECT);
    }

    #[test]
    fn encode_delete_object_request() {
        let req = DeleteObjectRequest {
            object_id: ObjectId::new(ObjectType::AnalogValue, 9),
            invoke_id: 5,
        };
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_DELETE_OBJECT);
    }

    #[test]
    fn decode_create_object_ack() {
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 3,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_CREATE_OBJECT,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogValue, 9).raw()).unwrap();
        let mut r = Reader::new(w.as_written());
        let _ack_hdr = ComplexAckHeader::decode(&mut r).unwrap();
        let ack = CreateObjectAck::decode_after_header(&mut r).unwrap();
        assert_eq!(ack.object_id, ObjectId::new(ObjectType::AnalogValue, 9));
    }
}
