use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{decode_signed, encode_app_object_id, encode_app_signed, encode_app_unsigned},
    reader::Reader,
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::types::ObjectId;
use crate::{DecodeError, EncodeError};

pub const SERVICE_ATOMIC_WRITE_FILE: u8 = 0x07;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicWriteFileAccessMethod<'a> {
    Stream {
        file_start_position: i32,
        file_data: &'a [u8],
    },
    Record {
        file_start_record: i32,
        file_record_data: &'a [&'a [u8]],
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicWriteFileRequest<'a> {
    pub file_object_id: ObjectId,
    pub access_method: AtomicWriteFileAccessMethod<'a>,
    pub invoke_id: u8,
}

impl<'a> AtomicWriteFileRequest<'a> {
    pub fn stream(
        file_object_id: ObjectId,
        file_start_position: i32,
        file_data: &'a [u8],
        invoke_id: u8,
    ) -> Self {
        Self {
            file_object_id,
            access_method: AtomicWriteFileAccessMethod::Stream {
                file_start_position,
                file_data,
            },
            invoke_id,
        }
    }

    pub fn record(
        file_object_id: ObjectId,
        file_start_record: i32,
        file_record_data: &'a [&'a [u8]],
        invoke_id: u8,
    ) -> Self {
        Self {
            file_object_id,
            access_method: AtomicWriteFileAccessMethod::Record {
                file_start_record,
                file_record_data,
            },
            invoke_id,
        }
    }

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
            service_choice: SERVICE_ATOMIC_WRITE_FILE,
        }
        .encode(w)?;

        encode_app_object_id(w, self.file_object_id.raw())?;
        match self.access_method {
            AtomicWriteFileAccessMethod::Stream {
                file_start_position,
                file_data,
            } => {
                Tag::Opening { tag_num: 0 }.encode(w)?;
                encode_app_signed(w, file_start_position)?;
                Tag::Application {
                    tag: AppTag::OctetString,
                    len: file_data.len() as u32,
                }
                .encode(w)?;
                w.write_all(file_data)?;
                Tag::Closing { tag_num: 0 }.encode(w)?;
            }
            AtomicWriteFileAccessMethod::Record {
                file_start_record,
                file_record_data,
            } => {
                Tag::Opening { tag_num: 1 }.encode(w)?;
                encode_app_signed(w, file_start_record)?;
                encode_app_unsigned(w, file_record_data.len() as u32)?;
                for record in file_record_data {
                    Tag::Application {
                        tag: AppTag::OctetString,
                        len: record.len() as u32,
                    }
                    .encode(w)?;
                    w.write_all(record)?;
                }
                Tag::Closing { tag_num: 1 }.encode(w)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicWriteFileAck {
    Stream { file_start_position: i32 },
    Record { file_start_record: i32 },
}

impl AtomicWriteFileAck {
    pub fn decode_after_header(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match Tag::decode(r)? {
            Tag::Context { tag_num: 0, len } => Ok(Self::Stream {
                file_start_position: decode_signed(r, len as usize)?,
            }),
            Tag::Context { tag_num: 1, len } => Ok(Self::Record {
                file_start_record: decode_signed(r, len as usize)?,
            }),
            Tag::Opening { tag_num: 0 } => {
                let start = decode_required_ctx_signed(r, 0)?;
                match Tag::decode(r)? {
                    Tag::Closing { tag_num: 0 } => Ok(Self::Stream {
                        file_start_position: start,
                    }),
                    _ => Err(DecodeError::InvalidTag),
                }
            }
            Tag::Opening { tag_num: 1 } => {
                let start = decode_required_ctx_signed(r, 0)?;
                match Tag::decode(r)? {
                    Tag::Closing { tag_num: 1 } => Ok(Self::Record {
                        file_start_record: start,
                    }),
                    _ => Err(DecodeError::InvalidTag),
                }
            }
            _ => Err(DecodeError::InvalidTag),
        }
    }
}

fn decode_required_ctx_signed(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<i32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context { tag_num, len } if tag_num == expected_tag_num => {
            decode_signed(r, len as usize)
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicWriteFileAck, AtomicWriteFileRequest, SERVICE_ATOMIC_WRITE_FILE};
    use crate::apdu::{ComplexAckHeader, ConfirmedRequestHeader};
    use crate::encoding::{reader::Reader, tag::Tag, writer::Writer};
    use crate::types::{ObjectId, ObjectType};

    #[test]
    fn encode_atomic_write_file_stream_request() {
        let req = AtomicWriteFileRequest::stream(
            ObjectId::new(ObjectType::File, 3),
            128,
            &[0xAA, 0xBB, 0xCC],
            5,
        );
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_ATOMIC_WRITE_FILE);
    }

    #[test]
    fn decode_atomic_write_file_ack_stream() {
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 2,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ATOMIC_WRITE_FILE,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Context { tag_num: 0, len: 2 }.encode(&mut w).unwrap();
        w.write_all(&0x0080u16.to_be_bytes()).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack = ComplexAckHeader::decode(&mut r).unwrap();
        let parsed = AtomicWriteFileAck::decode_after_header(&mut r).unwrap();
        assert_eq!(
            parsed,
            AtomicWriteFileAck::Stream {
                file_start_position: 128
            }
        );
    }
}
