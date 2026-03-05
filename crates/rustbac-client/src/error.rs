use rustbac_core::types::{ErrorClass, ErrorCode};
use rustbac_datalink::DataLinkError;
use thiserror::Error;

/// Errors returned by [`BacnetClient`](crate::BacnetClient) operations.
#[derive(Debug, Error)]
pub enum ClientError {
    /// The underlying transport layer returned an error (send, receive, or bind failure).
    #[error("datalink error: {0}")]
    DataLink(#[from] DataLinkError),
    /// An APDU or NPDU could not be encoded into the output buffer.
    #[error("encode error: {0}")]
    Encode(#[from] rustbac_core::EncodeError),
    /// An APDU or NPDU received from the network could not be decoded.
    #[error("decode error: {0}")]
    Decode(#[from] rustbac_core::DecodeError),
    /// No response was received from the remote device within the configured timeout.
    #[error("request timed out")]
    Timeout,
    /// The remote device responded with a BACnet Error PDU for `service_choice`.
    ///
    /// The raw numeric error class and code are always present when the device sends them;
    /// the typed variants are `Some` only when the values are recognised by this library.
    #[error("remote service error for service choice {service_choice}")]
    RemoteServiceError {
        service_choice: u8,
        error_class_raw: Option<u32>,
        error_code_raw: Option<u32>,
        error_class: Option<ErrorClass>,
        error_code: Option<ErrorCode>,
    },
    /// The remote device rejected the request with the given BACnet reject reason code.
    #[error("remote reject reason {reason}")]
    RemoteReject { reason: u8 },
    /// The remote device (or router) aborted the transaction. `server` is `true` when
    /// the Abort PDU was sent by the server side.
    #[error("remote abort reason {reason} (server={server})")]
    RemoteAbort { reason: u8, server: bool },
    /// A segment-ACK with the negative-ACK bit set was received for `sequence_number`
    /// during a segmented confirmed request.
    #[error("segment ack negative for sequence {sequence_number}")]
    SegmentNegativeAck { sequence_number: u8 },
    /// The encoded request payload is too large to fit within 255 segments of the
    /// negotiated maximum APDU size.
    #[error("segmented request too large")]
    SegmentedRequestTooLarge,
    /// The reassembled segmented response exceeded the internal 1 MiB safety limit.
    #[error("response payload exceeded {limit} bytes")]
    ResponseTooLarge { limit: usize },
    /// The response received from the device was syntactically valid but not understood
    /// (e.g. unexpected APDU type, missing required fields, or unsupported segmentation).
    #[error("unsupported response")]
    UnsupportedResponse,
    /// A `CovManager` or other component attempted to spawn a Tokio task outside of a
    /// Tokio runtime context.
    #[error("no active Tokio runtime — build() must be called from within a tokio::Runtime")]
    NoTokioRuntime,
}
