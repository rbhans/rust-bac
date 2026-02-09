use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{
        decode_signed, decode_unsigned, encode_app_object_id, encode_app_signed,
        encode_app_unsigned,
    },
    tag::Tag,
    writer::Writer,
};
use crate::types::ObjectId;
use crate::EncodeError;

#[cfg(feature = "alloc")]
use crate::encoding::reader::Reader;
#[cfg(feature = "alloc")]
use crate::encoding::tag::AppTag;
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_ATOMIC_READ_FILE: u8 = 0x06;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicReadFileAccessMethod {
    Stream {
        file_start_position: i32,
        requested_octet_count: u32,
    },
    Record {
        file_start_record: i32,
        requested_record_count: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicReadFileRequest {
    pub file_object_id: ObjectId,
    pub access_method: AtomicReadFileAccessMethod,
    pub invoke_id: u8,
}

impl AtomicReadFileRequest {
    pub fn stream(
        file_object_id: ObjectId,
        file_start_position: i32,
        requested_octet_count: u32,
        invoke_id: u8,
    ) -> Self {
        Self {
            file_object_id,
            access_method: AtomicReadFileAccessMethod::Stream {
                file_start_position,
                requested_octet_count,
            },
            invoke_id,
        }
    }

    pub fn record(
        file_object_id: ObjectId,
        file_start_record: i32,
        requested_record_count: u32,
        invoke_id: u8,
    ) -> Self {
        Self {
            file_object_id,
            access_method: AtomicReadFileAccessMethod::Record {
                file_start_record,
                requested_record_count,
            },
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
            service_choice: SERVICE_ATOMIC_READ_FILE,
        }
        .encode(w)?;

        encode_app_object_id(w, self.file_object_id.raw())?;
        match self.access_method {
            AtomicReadFileAccessMethod::Stream {
                file_start_position,
                requested_octet_count,
            } => {
                Tag::Opening { tag_num: 0 }.encode(w)?;
                encode_app_signed(w, file_start_position)?;
                encode_app_unsigned(w, requested_octet_count)?;
                Tag::Closing { tag_num: 0 }.encode(w)?;
            }
            AtomicReadFileAccessMethod::Record {
                file_start_record,
                requested_record_count,
            } => {
                Tag::Opening { tag_num: 1 }.encode(w)?;
                encode_app_signed(w, file_start_record)?;
                encode_app_unsigned(w, requested_record_count)?;
                Tag::Closing { tag_num: 1 }.encode(w)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicReadFileAckAccess<'a> {
    Stream {
        file_start_position: i32,
        file_data: &'a [u8],
    },
    Record {
        file_start_record: i32,
        returned_record_count: u32,
        file_record_data: Vec<&'a [u8]>,
    },
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicReadFileAck<'a> {
    pub end_of_file: bool,
    pub access_method: AtomicReadFileAckAccess<'a>,
}

#[cfg(feature = "alloc")]
impl<'a> AtomicReadFileAck<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let end_of_file = match Tag::decode(r)? {
            Tag::Application {
                tag: AppTag::Boolean,
                len,
            } => len != 0,
            _ => return Err(DecodeError::InvalidTag),
        };

        let access_method = match Tag::decode(r)? {
            Tag::Opening { tag_num: 0 } => {
                let file_start_position = decode_required_app_signed(r)?;
                let file_data = decode_required_app_octets(r)?;
                match Tag::decode(r)? {
                    Tag::Closing { tag_num: 0 } => {}
                    _ => return Err(DecodeError::InvalidTag),
                }
                AtomicReadFileAckAccess::Stream {
                    file_start_position,
                    file_data,
                }
            }
            Tag::Opening { tag_num: 1 } => {
                let file_start_record = decode_required_app_signed(r)?;
                let returned_record_count = decode_required_app_unsigned(r)?;

                let mut file_record_data = Vec::new();
                loop {
                    let tag = Tag::decode(r)?;
                    if tag == (Tag::Closing { tag_num: 1 }) {
                        break;
                    }
                    match tag {
                        Tag::Application {
                            tag: AppTag::OctetString,
                            len,
                        } => file_record_data.push(r.read_exact(len as usize)?),
                        _ => return Err(DecodeError::InvalidTag),
                    }
                }
                AtomicReadFileAckAccess::Record {
                    file_start_record,
                    returned_record_count,
                    file_record_data,
                }
            }
            _ => return Err(DecodeError::InvalidTag),
        };

        Ok(Self {
            end_of_file,
            access_method,
        })
    }
}

#[cfg(feature = "alloc")]
fn decode_required_app_unsigned(r: &mut Reader<'_>) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len,
        } => decode_unsigned(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_required_app_signed(r: &mut Reader<'_>) -> Result<i32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::SignedInt,
            len,
        } => decode_signed(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(feature = "alloc")]
fn decode_required_app_octets<'a>(r: &mut Reader<'a>) -> Result<&'a [u8], DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::OctetString,
            len,
        } => r.read_exact(len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "alloc")]
    use super::{AtomicReadFileAck, AtomicReadFileAckAccess};
    use super::{AtomicReadFileRequest, SERVICE_ATOMIC_READ_FILE};
    #[cfg(feature = "alloc")]
    use crate::apdu::ComplexAckHeader;
    use crate::apdu::ConfirmedRequestHeader;
    #[cfg(feature = "alloc")]
    use crate::encoding::tag::{AppTag, Tag};
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn encode_atomic_read_file_stream_request() {
        let req = AtomicReadFileRequest::stream(ObjectId::new(ObjectType::File, 7), 0, 512, 4);
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_ATOMIC_READ_FILE);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_atomic_read_file_ack_stream() {
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 9,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ATOMIC_READ_FILE,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Application {
            tag: AppTag::Boolean,
            len: 1,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Application {
            tag: AppTag::SignedInt,
            len: 1,
        }
        .encode(&mut w)
        .unwrap();
        w.write_u8(0).unwrap();
        Tag::Application {
            tag: AppTag::OctetString,
            len: 4,
        }
        .encode(&mut w)
        .unwrap();
        w.write_all(&[1, 2, 3, 4]).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack = ComplexAckHeader::decode(&mut r).unwrap();
        let parsed = AtomicReadFileAck::decode_after_header(&mut r).unwrap();
        assert!(parsed.end_of_file);
        match parsed.access_method {
            AtomicReadFileAckAccess::Stream {
                file_start_position,
                file_data,
            } => {
                assert_eq!(file_start_position, 0);
                assert_eq!(file_data, &[1, 2, 3, 4]);
            }
            other => panic!("unexpected ack access: {other:?}"),
        }
    }
}
