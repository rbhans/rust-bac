use crate::apdu::ApduType;
use crate::encoding::{
    primitives::decode_unsigned,
    reader::Reader,
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::{DecodeError, EncodeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmedRequestHeader {
    pub segmented: bool,
    pub more_follows: bool,
    pub segmented_response_accepted: bool,
    pub max_segments: u8,
    pub max_apdu: u8,
    pub invoke_id: u8,
    pub sequence_number: Option<u8>,
    pub proposed_window_size: Option<u8>,
    pub service_choice: u8,
}

impl ConfirmedRequestHeader {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        let mut b0 = (ApduType::ConfirmedRequest as u8) << 4;
        if self.segmented {
            b0 |= 0b0000_1000;
        }
        if self.more_follows {
            b0 |= 0b0000_0100;
        }
        if self.segmented_response_accepted {
            b0 |= 0b0000_0010;
        }

        w.write_u8(b0)?;
        w.write_u8((self.max_segments << 4) | (self.max_apdu & 0x0f))?;
        w.write_u8(self.invoke_id)?;
        if self.segmented {
            w.write_u8(self.sequence_number.unwrap_or(0))?;
            w.write_u8(self.proposed_window_size.unwrap_or(1))?;
        }
        w.write_u8(self.service_choice)?;
        Ok(())
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::ConfirmedRequest as u8 {
            return Err(DecodeError::InvalidValue);
        }
        let segmented = (b0 & 0b0000_1000) != 0;
        let more_follows = (b0 & 0b0000_0100) != 0;
        let segmented_response_accepted = (b0 & 0b0000_0010) != 0;
        let seg_apdu = r.read_u8()?;
        let invoke_id = r.read_u8()?;
        let (sequence_number, proposed_window_size) = if segmented {
            (Some(r.read_u8()?), Some(r.read_u8()?))
        } else {
            (None, None)
        };
        let service_choice = r.read_u8()?;
        Ok(Self {
            segmented,
            more_follows,
            segmented_response_accepted,
            max_segments: seg_apdu >> 4,
            max_apdu: seg_apdu & 0x0f,
            invoke_id,
            sequence_number,
            proposed_window_size,
            service_choice,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComplexAckHeader {
    pub segmented: bool,
    pub more_follows: bool,
    pub invoke_id: u8,
    pub sequence_number: Option<u8>,
    pub proposed_window_size: Option<u8>,
    pub service_choice: u8,
}

impl ComplexAckHeader {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        let mut b0 = (ApduType::ComplexAck as u8) << 4;
        if self.segmented {
            b0 |= 0b0000_1000;
        }
        if self.more_follows {
            b0 |= 0b0000_0100;
        }
        w.write_u8(b0)?;
        w.write_u8(self.invoke_id)?;
        if self.segmented {
            w.write_u8(self.sequence_number.unwrap_or(0))?;
            w.write_u8(self.proposed_window_size.unwrap_or(1))?;
        }
        w.write_u8(self.service_choice)
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::ComplexAck as u8 {
            return Err(DecodeError::InvalidValue);
        }

        let segmented = (b0 & 0b0000_1000) != 0;
        let more_follows = (b0 & 0b0000_0100) != 0;
        let invoke_id = r.read_u8()?;
        let (sequence_number, proposed_window_size) = if segmented {
            (Some(r.read_u8()?), Some(r.read_u8()?))
        } else {
            (None, None)
        };
        let service_choice = r.read_u8()?;

        Ok(Self {
            segmented,
            more_follows,
            invoke_id,
            sequence_number,
            proposed_window_size,
            service_choice,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleAck {
    pub invoke_id: u8,
    pub service_choice: u8,
}

impl SimpleAck {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.write_u8((ApduType::SimpleAck as u8) << 4)?;
        w.write_u8(self.invoke_id)?;
        w.write_u8(self.service_choice)
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::SimpleAck as u8 {
            return Err(DecodeError::InvalidValue);
        }
        Ok(Self {
            invoke_id: r.read_u8()?,
            service_choice: r.read_u8()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacnetError {
    pub invoke_id: u8,
    pub service_choice: u8,
    pub error_class: Option<u32>,
    pub error_code: Option<u32>,
}

impl BacnetError {
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::Error as u8 {
            return Err(DecodeError::InvalidValue);
        }
        let invoke_id = r.read_u8()?;
        let service_choice = r.read_u8()?;
        let mut error_class = None;
        let mut error_code = None;
        if !r.is_empty() {
            match Tag::decode(r)? {
                Tag::Opening { tag_num: 0 } => {
                    let class_tag = Tag::decode(r)?;
                    error_class = Some(decode_bacnet_error_value(r, class_tag, 0)?);
                    let code_tag = Tag::decode(r)?;
                    error_code = Some(decode_bacnet_error_value(r, code_tag, 1)?);
                    match Tag::decode(r)? {
                        Tag::Closing { tag_num: 0 } => {}
                        _ => return Err(DecodeError::InvalidTag),
                    }
                }
                first_tag => {
                    error_class = Some(decode_bacnet_error_value(r, first_tag, 0)?);
                    let second_tag = Tag::decode(r)?;
                    error_code = Some(decode_bacnet_error_value(r, second_tag, 1)?);
                }
            }
        }
        Ok(Self {
            invoke_id,
            service_choice,
            error_class,
            error_code,
        })
    }
}

fn decode_bacnet_error_value(
    r: &mut Reader<'_>,
    tag: Tag,
    expected_ctx_tag: u8,
) -> Result<u32, DecodeError> {
    match tag {
        Tag::Context { tag_num, len } if tag_num == expected_ctx_tag => {
            decode_unsigned(r, len as usize)
        }
        Tag::Application {
            tag: AppTag::Enumerated,
            len,
        } => decode_unsigned(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RejectPdu {
    pub invoke_id: u8,
    pub reason: u8,
}

impl RejectPdu {
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::Reject as u8 {
            return Err(DecodeError::InvalidValue);
        }
        Ok(Self {
            invoke_id: r.read_u8()?,
            reason: r.read_u8()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbortPdu {
    pub server: bool,
    pub invoke_id: u8,
    pub reason: u8,
}

impl AbortPdu {
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::Abort as u8 {
            return Err(DecodeError::InvalidValue);
        }
        Ok(Self {
            server: (b0 & 0x01) != 0,
            invoke_id: r.read_u8()?,
            reason: r.read_u8()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentAck {
    pub negative_ack: bool,
    pub sent_by_server: bool,
    pub invoke_id: u8,
    pub sequence_number: u8,
    pub actual_window_size: u8,
}

impl SegmentAck {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        let mut b0 = (ApduType::SegmentAck as u8) << 4;
        if self.negative_ack {
            b0 |= 0b0000_0010;
        }
        if self.sent_by_server {
            b0 |= 0b0000_0001;
        }
        w.write_u8(b0)?;
        w.write_u8(self.invoke_id)?;
        w.write_u8(self.sequence_number)?;
        w.write_u8(self.actual_window_size)?;
        Ok(())
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::SegmentAck as u8 {
            return Err(DecodeError::InvalidValue);
        }
        Ok(Self {
            negative_ack: (b0 & 0b0000_0010) != 0,
            sent_by_server: (b0 & 0b0000_0001) != 0,
            invoke_id: r.read_u8()?,
            sequence_number: r.read_u8()?,
            actual_window_size: r.read_u8()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BacnetError;
    use crate::encoding::reader::Reader;

    #[test]
    fn bacnet_error_decodes_without_details() {
        let mut r = Reader::new(&[0x50, 1, 15]);
        let e = BacnetError::decode(&mut r).unwrap();
        assert_eq!(e.invoke_id, 1);
        assert_eq!(e.service_choice, 15);
        assert_eq!(e.error_class, None);
        assert_eq!(e.error_code, None);
    }

    #[test]
    fn bacnet_error_decodes_with_details() {
        let mut r = Reader::new(&[0x50, 1, 15, 0x09, 0x02, 0x19, 0x20]);
        let e = BacnetError::decode(&mut r).unwrap();
        assert_eq!(e.invoke_id, 1);
        assert_eq!(e.service_choice, 15);
        assert_eq!(e.error_class, Some(2));
        assert_eq!(e.error_code, Some(32));
    }

    #[test]
    fn bacnet_error_decodes_application_enumerated_details() {
        let mut r = Reader::new(&[0x50, 1, 15, 0x91, 0x02, 0x91, 0x20]);
        let e = BacnetError::decode(&mut r).unwrap();
        assert_eq!(e.invoke_id, 1);
        assert_eq!(e.service_choice, 15);
        assert_eq!(e.error_class, Some(2));
        assert_eq!(e.error_code, Some(32));
    }

    #[test]
    fn bacnet_error_decodes_opening_wrapped_application_details() {
        let mut r = Reader::new(&[0x50, 1, 15, 0x0E, 0x91, 0x02, 0x91, 0x20, 0x0F]);
        let e = BacnetError::decode(&mut r).unwrap();
        assert_eq!(e.invoke_id, 1);
        assert_eq!(e.service_choice, 15);
        assert_eq!(e.error_class, Some(2));
        assert_eq!(e.error_code, Some(32));
    }
}
