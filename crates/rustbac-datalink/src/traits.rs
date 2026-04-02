#[cfg(feature = "std")]
use crate::DataLinkAddress;

/// Errors that can occur at the data-link layer.
#[derive(Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum DataLinkError {
    #[cfg(feature = "std")]
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg_attr(feature = "std", error("frame too large"))]
    FrameTooLarge,
    #[cfg_attr(feature = "std", error("invalid frame"))]
    InvalidFrame,
    #[cfg_attr(feature = "std", error("unsupported BVLC function 0x{0:02x}"))]
    UnsupportedBvlcFunction(u8),
    #[cfg_attr(feature = "std", error("BVLC result code 0x{0:04x}"))]
    BvlcResult(u16),
    #[cfg_attr(feature = "std", error("bbmd not configured"))]
    BbmdNotConfigured,
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for DataLinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DataLinkError::FrameTooLarge => write!(f, "frame too large"),
            DataLinkError::InvalidFrame => write!(f, "invalid frame"),
            DataLinkError::UnsupportedBvlcFunction(v) => {
                write!(f, "unsupported BVLC function 0x{v:02x}")
            }
            DataLinkError::BvlcResult(v) => write!(f, "BVLC result code 0x{v:04x}"),
            DataLinkError::BbmdNotConfigured => write!(f, "bbmd not configured"),
        }
    }
}

/// Async trait for sending and receiving raw BACnet frames.
///
/// Implementors include [`BacnetIpTransport`](crate::BacnetIpTransport) for
/// BACnet/IP over UDP and [`BacnetScTransport`] for BACnet/SC over WebSocket.
#[cfg(feature = "std")]
pub trait DataLink: Send + Sync {
    /// Sends `payload` to the given data-link `address`.
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError>;

    /// Receives a frame into `buf`, returning `(bytes_read, source_address)`.
    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError>;
}
