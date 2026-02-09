use crate::apdu::UnconfirmedRequestHeader;
use crate::encoding::primitives::encode_ctx_unsigned;
use crate::encoding::writer::Writer;
use crate::EncodeError;

pub const SERVICE_WHO_IS: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhoIsRequest {
    pub low_limit: Option<u32>,
    pub high_limit: Option<u32>,
}

impl WhoIsRequest {
    pub const fn global() -> Self {
        Self {
            low_limit: None,
            high_limit: None,
        }
    }

    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        UnconfirmedRequestHeader {
            service_choice: SERVICE_WHO_IS,
        }
        .encode(w)?;

        if let Some(low) = self.low_limit {
            encode_ctx_unsigned(w, 0, low)?;
        }
        if let Some(high) = self.high_limit {
            encode_ctx_unsigned(w, 1, high)?;
        }
        Ok(())
    }
}
