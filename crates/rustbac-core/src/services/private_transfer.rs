use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{primitives::encode_ctx_unsigned, tag::Tag, writer::Writer};
use crate::EncodeError;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader};
#[cfg(feature = "alloc")]
use crate::DecodeError;

pub const SERVICE_CONFIRMED_PRIVATE_TRANSFER: u8 = 18;
pub const SERVICE_UNCONFIRMED_PRIVATE_TRANSFER: u8 = 4;

/// A ConfirmedPrivateTransfer request as defined in clause 16.
///
/// Vendor-specific data is passed as an opaque byte slice in `service_parameters`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedPrivateTransferRequest<'a> {
    pub vendor_id: u32,
    pub service_number: u32,
    pub service_parameters: Option<&'a [u8]>,
    pub invoke_id: u8,
}

impl<'a> ConfirmedPrivateTransferRequest<'a> {
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
            service_choice: SERVICE_CONFIRMED_PRIVATE_TRANSFER,
        }
        .encode(w)?;

        // [0] vendor-id
        encode_ctx_unsigned(w, 0, self.vendor_id)?;
        // [1] service-number
        encode_ctx_unsigned(w, 1, self.service_number)?;
        // [2] service-parameters (optional, constructed)
        if let Some(params) = self.service_parameters {
            Tag::Opening { tag_num: 2 }.encode(w)?;
            w.write_all(params)?;
            Tag::Closing { tag_num: 2 }.encode(w)?;
        }
        Ok(())
    }
}

/// The ack (result) from a ConfirmedPrivateTransfer.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedPrivateTransferAck {
    pub vendor_id: u32,
    pub service_number: u32,
    pub result_block: Option<Vec<u8>>,
}

#[cfg(feature = "alloc")]
impl ConfirmedPrivateTransferAck {
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        // [0] vendor-id
        let vendor_id = decode_ctx_unsigned(r, 0)?;
        // [1] service-number
        let service_number = decode_ctx_unsigned(r, 1)?;
        // [2] result-block (optional, constructed)
        let result_block = if !r.is_empty() {
            match Tag::decode(r)? {
                Tag::Opening { tag_num: 2 } => Some(decode_constructed_block_inner_bytes(r, 2)?),
                _ => return Err(DecodeError::InvalidTag),
            }
        } else {
            None
        };
        if !r.is_empty() {
            return Err(DecodeError::InvalidTag);
        }
        Ok(Self {
            vendor_id,
            service_number,
            result_block,
        })
    }
}

#[cfg(feature = "alloc")]
fn decode_constructed_block_inner_bytes(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<Vec<u8>, DecodeError> {
    // Scan with a probe reader to locate the matching closing tag while validating nesting.
    let start = r.position();
    let mut probe = *r;
    let mut stack = Vec::new();
    loop {
        let tag_start = probe.position();
        let tag = Tag::decode(&mut probe)?;
        match tag {
            Tag::Application {
                tag: crate::encoding::tag::AppTag::Boolean,
                ..
            } => {}
            Tag::Application { len, .. } | Tag::Context { len, .. } => {
                probe.read_exact(len as usize)?;
            }
            Tag::Opening { tag_num } => stack.push(tag_num),
            Tag::Closing { tag_num } => {
                if let Some(opening) = stack.pop() {
                    if opening != tag_num {
                        return Err(DecodeError::InvalidTag);
                    }
                } else if tag_num == expected_tag_num {
                    let inner_len = tag_start
                        .checked_sub(start)
                        .ok_or(DecodeError::InvalidLength)?;
                    let inner = r.read_exact(inner_len)?.to_vec();
                    match Tag::decode(r)? {
                        Tag::Closing { tag_num: closing } if closing == expected_tag_num => {}
                        _ => return Err(DecodeError::InvalidTag),
                    }
                    return Ok(inner);
                } else {
                    return Err(DecodeError::InvalidTag);
                }
            }
        }
    }
}

#[cfg(feature = "alloc")]
fn decode_ctx_unsigned(r: &mut Reader<'_>, expected_tag_num: u8) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context { tag_num, len } if tag_num == expected_tag_num => {
            decode_unsigned(r, len as usize)
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apdu::ConfirmedRequestHeader;
    #[cfg(feature = "alloc")]
    use crate::encoding::{primitives::encode_ctx_unsigned, tag::AppTag};
    use crate::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn encode_private_transfer_request() {
        let req = ConfirmedPrivateTransferRequest {
            vendor_id: 42,
            service_number: 1,
            service_parameters: Some(&[0x01, 0x02, 0x03]),
            invoke_id: 5,
        };

        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.invoke_id, 5);
        assert_eq!(header.service_choice, SERVICE_CONFIRMED_PRIVATE_TRANSFER);
        assert!(!r.is_empty());
    }

    #[test]
    fn encode_private_transfer_no_params() {
        let req = ConfirmedPrivateTransferRequest {
            vendor_id: 100,
            service_number: 7,
            service_parameters: None,
            invoke_id: 1,
        };

        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.service_choice, SERVICE_CONFIRMED_PRIVATE_TRANSFER);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_private_transfer_ack_preserves_nested_result_block_bytes() {
        let mut payload = [0u8; 128];
        let mut w = Writer::new(&mut payload);
        encode_ctx_unsigned(&mut w, 0, 77).unwrap();
        encode_ctx_unsigned(&mut w, 1, 9).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();

        let mut expected_inner = [0u8; 64];
        let mut ew = Writer::new(&mut expected_inner);
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len: 1,
        }
        .encode(&mut ew)
        .unwrap();
        ew.write_u8(0x2A).unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut ew).unwrap();
        Tag::Application {
            tag: AppTag::Boolean,
            len: 1,
        }
        .encode(&mut ew)
        .unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut ew).unwrap();
        w.write_all(ew.as_written()).unwrap();

        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let ack = ConfirmedPrivateTransferAck::decode(&mut r).unwrap();
        assert_eq!(ack.vendor_id, 77);
        assert_eq!(ack.service_number, 9);
        assert_eq!(ack.result_block, Some(ew.as_written().to_vec()));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_private_transfer_ack_rejects_trailing_or_invalid_optional_block() {
        let mut invalid_optional = [0u8; 64];
        let mut w = Writer::new(&mut invalid_optional);
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        encode_ctx_unsigned(&mut w, 1, 2).unwrap();
        encode_ctx_unsigned(&mut w, 3, 9).unwrap();
        let mut r = Reader::new(w.as_written());
        assert_eq!(
            ConfirmedPrivateTransferAck::decode(&mut r).unwrap_err(),
            DecodeError::InvalidTag
        );

        let mut trailing = [0u8; 64];
        let mut w = Writer::new(&mut trailing);
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        encode_ctx_unsigned(&mut w, 1, 2).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 3, 9).unwrap();
        let mut r = Reader::new(w.as_written());
        assert_eq!(
            ConfirmedPrivateTransferAck::decode(&mut r).unwrap_err(),
            DecodeError::InvalidTag
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_private_transfer_ack_distinguishes_absent_vs_empty_result_block() {
        let mut payload = [0u8; 64];
        let mut w = Writer::new(&mut payload);
        encode_ctx_unsigned(&mut w, 0, 11).unwrap();
        encode_ctx_unsigned(&mut w, 1, 22).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let ack = ConfirmedPrivateTransferAck::decode(&mut r).unwrap();
        assert_eq!(ack.result_block, Some(Vec::new()));
    }
}
