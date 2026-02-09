use crate::DataLinkAddress;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataLinkError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame too large")]
    FrameTooLarge,
    #[error("invalid frame")]
    InvalidFrame,
    #[error("unsupported BVLC function 0x{0:02x}")]
    UnsupportedBvlcFunction(u8),
    #[error("BVLC result code 0x{0:04x}")]
    BvlcResult(u16),
    #[error("bbmd not configured")]
    BbmdNotConfigured,
}

pub trait DataLink: Send + Sync {
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError>;

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError>;
}
