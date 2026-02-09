use crate::apdu::UnconfirmedRequestHeader;
use crate::encoding::{
    primitives::{
        decode_ctx_character_string, decode_unsigned, encode_ctx_character_string,
        encode_ctx_object_id, encode_ctx_unsigned,
    },
    reader::Reader,
    tag::Tag,
    writer::Writer,
};
use crate::types::ObjectId;
use crate::{DecodeError, EncodeError};

pub const SERVICE_I_HAVE: u8 = 0x01;
pub const SERVICE_WHO_HAS: u8 = 0x07;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhoHasObject<'a> {
    ObjectId(ObjectId),
    ObjectName(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhoHasRequest<'a> {
    pub low_limit: Option<u32>,
    pub high_limit: Option<u32>,
    pub object: WhoHasObject<'a>,
}

impl<'a> WhoHasRequest<'a> {
    pub const fn for_object_id(object_id: ObjectId) -> Self {
        Self {
            low_limit: None,
            high_limit: None,
            object: WhoHasObject::ObjectId(object_id),
        }
    }

    pub const fn for_object_name(object_name: &'a str) -> Self {
        Self {
            low_limit: None,
            high_limit: None,
            object: WhoHasObject::ObjectName(object_name),
        }
    }

    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        UnconfirmedRequestHeader {
            service_choice: SERVICE_WHO_HAS,
        }
        .encode(w)?;

        match (self.low_limit, self.high_limit) {
            (Some(low), Some(high)) => {
                encode_ctx_unsigned(w, 0, low)?;
                encode_ctx_unsigned(w, 1, high)?;
            }
            (None, None) => {}
            _ => {
                return Err(EncodeError::Message(
                    "low/high limits must be both set or absent",
                ))
            }
        }

        match self.object {
            WhoHasObject::ObjectId(object_id) => encode_ctx_object_id(w, 2, object_id.raw()),
            WhoHasObject::ObjectName(object_name) => encode_ctx_character_string(w, 3, object_name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IHaveRequest<'a> {
    pub device_id: ObjectId,
    pub object_id: ObjectId,
    pub object_name: &'a str,
}

impl<'a> IHaveRequest<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let device_id = decode_required_ctx_object_id(r, 0)?;
        let object_id = decode_required_ctx_object_id(r, 1)?;
        let object_name = match Tag::decode(r)? {
            Tag::Context { tag_num: 2, len } => decode_ctx_character_string(r, len as usize)?,
            _ => return Err(DecodeError::InvalidTag),
        };
        Ok(Self {
            device_id,
            object_id,
            object_name,
        })
    }
}

fn decode_required_ctx_object_id(
    r: &mut Reader<'_>,
    expected_tag: u8,
) -> Result<ObjectId, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context { tag_num, len } if tag_num == expected_tag => {
            if len != 4 {
                return Err(DecodeError::InvalidLength);
            }
            Ok(ObjectId::from_raw(decode_unsigned(r, len as usize)?))
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(test)]
mod tests {
    use super::{IHaveRequest, WhoHasRequest, SERVICE_I_HAVE, SERVICE_WHO_HAS};
    use crate::apdu::UnconfirmedRequestHeader;
    use crate::encoding::{
        primitives::{encode_ctx_character_string, encode_ctx_object_id},
        reader::Reader,
        writer::Writer,
    };
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn encode_who_has_request_by_id() {
        let req = WhoHasRequest::for_object_id(ObjectId::new(ObjectType::AnalogInput, 2));
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_WHO_HAS);
    }

    #[test]
    fn encode_who_has_request_by_name_with_limits() {
        let req = WhoHasRequest {
            low_limit: Some(1),
            high_limit: Some(100),
            object: super::WhoHasObject::ObjectName("AHU-1"),
        };
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        assert_eq!(w.as_written()[0], 0x10);
    }

    #[test]
    fn decode_i_have_after_header() {
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        UnconfirmedRequestHeader {
            service_choice: SERVICE_I_HAVE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::Device, 5).raw()).unwrap();
        encode_ctx_object_id(&mut w, 1, ObjectId::new(ObjectType::AnalogInput, 2).raw()).unwrap();
        encode_ctx_character_string(&mut w, 2, "Zone Temp").unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_I_HAVE);
        let decoded = IHaveRequest::decode_after_header(&mut r).unwrap();
        assert_eq!(decoded.device_id, ObjectId::new(ObjectType::Device, 5));
        assert_eq!(decoded.object_id, ObjectId::new(ObjectType::AnalogInput, 2));
        assert_eq!(decoded.object_name, "Zone Temp");
    }

    #[test]
    fn encode_who_has_request_rejects_partial_limits() {
        let req = WhoHasRequest {
            low_limit: Some(1),
            high_limit: None,
            object: super::WhoHasObject::ObjectName("bad"),
        };
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        assert!(req.encode(&mut w).is_err());
    }
}
