use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_object_id, encode_ctx_unsigned},
    tag::Tag,
    writer::Writer,
};
use crate::types::{ObjectId, PropertyId};
use crate::EncodeError;

#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader};
#[cfg(feature = "alloc")]
use crate::services::value_codec::decode_application_data_value_from_tag;
#[cfg(feature = "alloc")]
use crate::types::DataValue;
#[cfg(feature = "alloc")]
use crate::DecodeError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub const SERVICE_READ_PROPERTY_MULTIPLE: u8 = 0x0E;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyReference {
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadAccessSpecification<'a> {
    pub object_id: ObjectId,
    pub properties: &'a [PropertyReference],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPropertyMultipleRequest<'a> {
    pub specs: &'a [ReadAccessSpecification<'a>],
    pub invoke_id: u8,
}

impl<'a> ReadPropertyMultipleRequest<'a> {
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
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
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
            }
            Tag::Closing { tag_num: 1 }.encode(w)?;
        }

        Ok(())
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct ReadResultElement<'a> {
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub value: DataValue<'a>,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAccessResult<'a> {
    pub object_id: ObjectId,
    pub results: Vec<ReadResultElement<'a>>,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct ReadPropertyMultipleAck<'a> {
    pub results: Vec<ReadAccessResult<'a>>,
}

#[cfg(feature = "alloc")]
impl<'a> ReadPropertyMultipleAck<'a> {
    pub fn decode_after_header(r: &mut Reader<'a>) -> Result<Self, DecodeError> {
        let mut all_results = Vec::new();

        while !r.is_empty() {
            let object_id = match Tag::decode(r)? {
                Tag::Context { tag_num: 0, len } => {
                    ObjectId::from_raw(decode_unsigned(r, len as usize)?)
                }
                _ => return Err(DecodeError::InvalidTag),
            };

            match Tag::decode(r)? {
                Tag::Opening { tag_num: 1 } => {}
                _ => return Err(DecodeError::InvalidTag),
            }

            let mut elements = Vec::new();
            loop {
                let tag = Tag::decode(r)?;
                if tag == (Tag::Closing { tag_num: 1 }) {
                    break;
                }

                let property_id = match tag {
                    Tag::Context { tag_num: 2, len } => {
                        PropertyId::from_u32(decode_unsigned(r, len as usize)?)
                    }
                    _ => return Err(DecodeError::InvalidTag),
                };

                let next = Tag::decode(r)?;
                let (array_index, read_result_open) = match next {
                    Tag::Context { tag_num: 3, len } => {
                        let idx = decode_unsigned(r, len as usize)?;
                        (Some(idx), Tag::decode(r)?)
                    }
                    other => (None, other),
                };

                if read_result_open != (Tag::Opening { tag_num: 4 }) {
                    return Err(DecodeError::InvalidTag);
                }

                let value_or_error = Tag::decode(r)?;
                let value = if value_or_error == (Tag::Opening { tag_num: 5 }) {
                    // Property access error block [5] with errorClass [0], errorCode [1].
                    // Phase 1: decode and surface as unsupported response.
                    let class_tag = Tag::decode(r)?;
                    let code_tag = Tag::decode(r)?;
                    let close_tag = Tag::decode(r)?;
                    match (class_tag, code_tag, close_tag) {
                        (
                            Tag::Context { tag_num: 0, .. },
                            Tag::Context { tag_num: 1, .. },
                            Tag::Closing { tag_num: 5 },
                        ) => return Err(DecodeError::Unsupported),
                        _ => return Err(DecodeError::InvalidTag),
                    }
                } else {
                    decode_application_data_value_from_tag(r, value_or_error)?
                };

                match Tag::decode(r)? {
                    Tag::Closing { tag_num: 4 } => {}
                    _ => return Err(DecodeError::InvalidTag),
                }

                elements.push(ReadResultElement {
                    property_id,
                    array_index,
                    value,
                });
            }

            all_results.push(ReadAccessResult {
                object_id,
                results: elements,
            });
        }

        Ok(Self {
            results: all_results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PropertyReference, ReadAccessSpecification, ReadPropertyMultipleRequest,
        SERVICE_READ_PROPERTY_MULTIPLE,
    };
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{ObjectId, ObjectType, PropertyId};

    #[test]
    fn encode_read_property_multiple_request() {
        let props = [
            PropertyReference {
                property_id: PropertyId::ObjectName,
                array_index: None,
            },
            PropertyReference {
                property_id: PropertyId::PresentValue,
                array_index: Some(1),
            },
        ];

        let specs = [ReadAccessSpecification {
            object_id: ObjectId::new(ObjectType::Device, 123),
            properties: &props,
        }];

        let req = ReadPropertyMultipleRequest {
            specs: &specs,
            invoke_id: 7,
        };

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let header = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(header.invoke_id, 7);
        assert_eq!(header.service_choice, SERVICE_READ_PROPERTY_MULTIPLE);
        assert!(!r.is_empty());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_read_property_multiple_ack_minimal() {
        use super::ReadPropertyMultipleAck;
        use crate::apdu::ComplexAckHeader;
        use crate::encoding::primitives::{encode_app_real, encode_ctx_unsigned};
        use crate::encoding::tag::Tag;

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 9,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w)
        .unwrap();

        encode_ctx_unsigned(&mut w, 0, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        Tag::Opening { tag_num: 1 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 2, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 42.0).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 1 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack = ComplexAckHeader::decode(&mut r).unwrap();
        let parsed = ReadPropertyMultipleAck::decode_after_header(&mut r).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].results.len(), 1);
    }
}
