use crate::DataLinkAddress;
use thiserror::Error;

/// Errors that can occur at the data-link layer.
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

/// Async trait for sending and receiving raw BACnet frames.
///
/// Implementors include [`BacnetIpTransport`](crate::BacnetIpTransport) for
/// BACnet/IP over UDP and [`BacnetScTransport`] for BACnet/SC over WebSocket.
pub trait DataLink: Send + Sync {
    /// Sends `payload` to the given data-link `address`.
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError>;

    /// Receives a frame into `buf`, returning `(bytes_read, source_address)`.
    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError>;
}
