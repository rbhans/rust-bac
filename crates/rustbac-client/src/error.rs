use rustbac_core::types::{ErrorClass, ErrorCode};
use rustbac_datalink::DataLinkError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("datalink error: {0}")]
    DataLink(#[from] DataLinkError),
    #[error("encode error: {0}")]
    Encode(#[from] rustbac_core::EncodeError),
    #[error("decode error: {0}")]
    Decode(#[from] rustbac_core::DecodeError),
    #[error("request timed out")]
    Timeout,
    #[error("remote service error for service choice {service_choice}")]
    RemoteServiceError {
        service_choice: u8,
        error_class_raw: Option<u32>,
        error_code_raw: Option<u32>,
        error_class: Option<ErrorClass>,
        error_code: Option<ErrorCode>,
    },
    #[error("remote reject reason {reason}")]
    RemoteReject { reason: u8 },
    #[error("remote abort reason {reason} (server={server})")]
    RemoteAbort { reason: u8, server: bool },
    #[error("segment ack negative for sequence {sequence_number}")]
    SegmentNegativeAck { sequence_number: u8 },
    #[error("segmented request too large")]
    SegmentedRequestTooLarge,
    #[error("response payload exceeded {limit} bytes")]
    ResponseTooLarge { limit: usize },
    #[error("unsupported response")]
    UnsupportedResponse,
}
