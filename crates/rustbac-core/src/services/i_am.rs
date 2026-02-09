use crate::apdu::UnconfirmedRequestHeader;
use crate::encoding::{
    primitives::{
        decode_app_enumerated, decode_app_unsigned, encode_app_enumerated, encode_app_unsigned,
    },
    reader::Reader,
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::types::ObjectId;
use crate::{DecodeError, EncodeError};

pub const SERVICE_I_AM: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IAmRequest {
    pub device_id: ObjectId,
    pub max_apdu: u32,
    pub segmentation: u32,
    pub vendor_id: u32,
}

impl IAmRequest {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        UnconfirmedRequestHeader {
            service_choice: SERVICE_I_AM,
        }
        .encode(w)?;

        Tag::Application {
            tag: AppTag::ObjectId,
            len: 4,
        }
        .encode(w)?;
        w.write_all(&self.device_id.raw().to_be_bytes())?;
        encode_app_unsigned(w, self.max_apdu)?;
        encode_app_enumerated(w, self.segmentation)?;
        encode_app_unsigned(w, self.vendor_id)?;
        Ok(())
    }

    pub fn decode_after_header(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let device_id = match Tag::decode(r)? {
            Tag::Application {
                tag: AppTag::ObjectId,
                len: 4,
            } => {
                let b = r.read_exact(4)?;
                ObjectId::from_raw(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            }
            _ => return Err(DecodeError::InvalidTag),
        };
        let max_apdu = decode_app_unsigned(r)?;
        let segmentation = decode_app_enumerated(r)?;
        let vendor_id = decode_app_unsigned(r)?;

        Ok(Self {
            device_id,
            max_apdu,
            segmentation,
            vendor_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::IAmRequest;
    use crate::apdu::UnconfirmedRequestHeader;
    use crate::encoding::{reader::Reader, tag::AppTag, tag::Tag, writer::Writer};
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn i_am_segmentation_is_enumerated() {
        let req = IAmRequest {
            device_id: ObjectId::new(ObjectType::Device, 1234),
            max_apdu: 1476,
            segmentation: 3,
            vendor_id: 260,
        };
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        let _obj = Tag::decode(&mut r).unwrap();
        let _obj_data = r.read_exact(4).unwrap();
        let _max = Tag::decode(&mut r).unwrap();
        let _max_data = r.read_exact(2).unwrap();
        let seg_tag = Tag::decode(&mut r).unwrap();
        assert_eq!(
            seg_tag,
            Tag::Application {
                tag: AppTag::Enumerated,
                len: 1
            }
        );
    }
}
