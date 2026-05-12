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

impl ReadPropertyMultipleRequest<'_> {
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

/// A propertyAccessError block carried inline in a ReadPropertyMultiple
/// readResult. Mirrors the BACnet `propertyAccessError [5]` CHOICE arm:
/// `errorClass [0] ENUMERATED`, `errorCode [1] ENUMERATED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyAccessError {
    pub error_class: u32,
    pub error_code: u32,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct ReadResultElement<'a> {
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    /// `Ok` carries the decoded value; `Err` preserves the
    /// `propertyAccessError [5]` block so callers can surface per-property
    /// failures without dropping sibling success results.
    pub value: Result<DataValue<'a>, PropertyAccessError>,
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

                let value = decode_read_result_value(r)?;

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

/// Decode the `[4] readResult` content for one element of a
/// ReadPropertyMultiple ACK, consuming the matching `Tag::Closing { tag_num: 4 }`.
///
/// The first tag determines the variant:
/// - `Tag::Opening { tag_num: 5 }` — propertyAccessError block; returns
///   `Ok(Err(PropertyAccessError { … }))` and consumes the enclosing
///   `[4]<close>` so the surrounding decoder can keep going.
/// - Anything else — accumulate values until `Tag::Closing { tag_num: 4 }`.
///   Single value returns directly; multiple values return
///   `DataValue::Constructed { tag_num: 4, values }`; zero values return
///   `DataValue::Null`.
#[cfg(feature = "alloc")]
fn decode_read_result_value<'a>(
    r: &mut Reader<'a>,
) -> Result<Result<DataValue<'a>, PropertyAccessError>, DecodeError> {
    let first = Tag::decode(r)?;
    if first == (Tag::Opening { tag_num: 5 }) {
        // propertyAccessError [5] errorClass [0] errorCode [1] [5]<close>.
        let error_class = match Tag::decode(r)? {
            Tag::Context { tag_num: 0, len } => decode_unsigned(r, len as usize)?,
            _ => return Err(DecodeError::InvalidTag),
        };
        let error_code = match Tag::decode(r)? {
            Tag::Context { tag_num: 1, len } => decode_unsigned(r, len as usize)?,
            _ => return Err(DecodeError::InvalidTag),
        };
        if Tag::decode(r)? != (Tag::Closing { tag_num: 5 }) {
            return Err(DecodeError::InvalidTag);
        }
        // Consume the outer [4]<close> so the surrounding decoder can pick up
        // the next property element instead of bailing.
        if Tag::decode(r)? != (Tag::Closing { tag_num: 4 }) {
            return Err(DecodeError::InvalidTag);
        }
        return Ok(Err(PropertyAccessError {
            error_class,
            error_code,
        }));
    }

    let mut values: Vec<DataValue<'a>> = Vec::new();
    values.push(decode_application_data_value_from_tag(r, first)?);
    loop {
        let next = Tag::decode(r)?;
        if next == (Tag::Closing { tag_num: 4 }) {
            return Ok(Ok(match values.len() {
                1 => values.into_iter().next().unwrap(),
                _ => DataValue::Constructed { tag_num: 4, values },
            }));
        }
        values.push(decode_application_data_value_from_tag(r, next)?);
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

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_read_property_multiple_ack_preserves_success_when_other_property_errors() {
        use super::{PropertyAccessError, ReadPropertyMultipleAck};
        use crate::apdu::ComplexAckHeader;
        use crate::encoding::primitives::{encode_app_real, encode_ctx_unsigned};
        use crate::encoding::tag::Tag;
        use crate::types::DataValue;

        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 12,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w)
        .unwrap();

        let oid = ObjectId::new(ObjectType::AnalogValue, 7);
        encode_ctx_unsigned(&mut w, 0, oid.raw()).unwrap();
        Tag::Opening { tag_num: 1 }.encode(&mut w).unwrap();

        // First property: present-value = 42.0 (success).
        encode_ctx_unsigned(&mut w, 2, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 42.0).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        // Second property: description, propertyAccessError(class=2/property, code=32/unknown-property).
        encode_ctx_unsigned(&mut w, 2, PropertyId::Description.to_u32()).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        Tag::Opening { tag_num: 5 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, 2).unwrap();
        encode_ctx_unsigned(&mut w, 1, 32).unwrap();
        Tag::Closing { tag_num: 5 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        // Third property: units = 95 (success — verifies decoder kept going past the error).
        encode_ctx_unsigned(&mut w, 2, PropertyId::Units.to_u32()).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        crate::encoding::primitives::encode_app_enumerated(&mut w, 95).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        Tag::Closing { tag_num: 1 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack = ComplexAckHeader::decode(&mut r).unwrap();
        let parsed = ReadPropertyMultipleAck::decode_after_header(&mut r).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].results.len(), 3);

        let elements = &parsed.results[0].results;
        assert_eq!(elements[0].property_id, PropertyId::PresentValue);
        assert!(matches!(elements[0].value, Ok(DataValue::Real(v)) if v == 42.0));

        assert_eq!(elements[1].property_id, PropertyId::Description);
        assert_eq!(
            elements[1].value,
            Err(PropertyAccessError {
                error_class: 2,
                error_code: 32,
            })
        );

        assert_eq!(elements[2].property_id, PropertyId::Units);
        assert!(matches!(elements[2].value, Ok(DataValue::Enumerated(95))));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn decode_read_property_multiple_ack_with_array_value() {
        use super::ReadPropertyMultipleAck;
        use crate::apdu::ComplexAckHeader;
        use crate::encoding::primitives::encode_ctx_unsigned;
        use crate::encoding::tag::{AppTag, Tag};
        use crate::types::DataValue;

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

        let device = ObjectId::new(ObjectType::Device, 1);
        let ao = ObjectId::new(ObjectType::AnalogOutput, 0);
        encode_ctx_unsigned(&mut w, 0, device.raw()).unwrap();
        Tag::Opening { tag_num: 1 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 2, PropertyId::ObjectList.to_u32()).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        for oid in &[device, ao] {
            Tag::Application {
                tag: AppTag::ObjectId,
                len: 4,
            }
            .encode(&mut w)
            .unwrap();
            w.write_all(&oid.raw().to_be_bytes()).unwrap();
        }
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 1 }.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let _ack = ComplexAckHeader::decode(&mut r).unwrap();
        let parsed = ReadPropertyMultipleAck::decode_after_header(&mut r).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].results.len(), 1);
        let elem = &parsed.results[0].results[0];
        assert_eq!(elem.property_id, PropertyId::ObjectList);
        match &elem.value {
            Ok(DataValue::Constructed { tag_num: 4, values }) => {
                assert_eq!(values.len(), 2);
                assert!(matches!(values[0], DataValue::ObjectId(o) if o == device));
                assert!(matches!(values[1], DataValue::ObjectId(o) if o == ao));
            }
            other => panic!("expected Ok(Constructed), got {other:?}"),
        }
    }
}
