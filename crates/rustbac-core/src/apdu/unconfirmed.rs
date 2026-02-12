use crate::apdu::ApduType;
use crate::encoding::{reader::Reader, writer::Writer};
use crate::{DecodeError, EncodeError};

/// Header for a BACnet Unconfirmed-Request APDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnconfirmedRequestHeader {
    pub service_choice: u8,
}

impl UnconfirmedRequestHeader {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.write_u8((ApduType::UnconfirmedRequest as u8) << 4)?;
        w.write_u8(self.service_choice)
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let b0 = r.read_u8()?;
        if (b0 >> 4) != ApduType::UnconfirmedRequest as u8 {
            return Err(DecodeError::InvalidValue);
        }
        Ok(Self {
            service_choice: r.read_u8()?,
        })
    }
}
