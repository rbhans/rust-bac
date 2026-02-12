//! High-level async BACnet client.
//!
//! [`BacnetClient`] wraps any [`DataLink`](rustbac_datalink::DataLink)
//! transport and exposes ergonomic methods for common BACnet operations
//! such as reading properties, discovering devices, and subscribing to
//! change-of-value (COV) notifications.

/// Alarm and event services (GetAlarmSummary, GetEventInformation, etc.).
pub mod alarm;
/// Core [`BacnetClient`] type and transport setup.
pub mod client;
/// Change-of-value (COV) notification types.
pub mod cov;
/// Device and object discovery (Who-Is / I-Am / Who-Has).
pub mod discovery;
/// Client-level error type.
pub mod error;
/// Atomic file read/write operations.
pub mod file;
/// ReadRange results and related types.
pub mod range;
/// Owned application-data values for client-side use.
pub mod value;

pub use alarm::{
    AlarmSummaryItem, EnrollmentSummaryItem, EventInformationItem, EventInformationResult,
    EventNotification,
};
pub use client::{BacnetClient, ForeignDeviceRenewal};
pub use cov::{CovNotification, CovPropertyValue};
pub use discovery::{DiscoveredDevice, DiscoveredObject};
pub use error::ClientError;
pub use file::{AtomicReadFileResult, AtomicWriteFileResult};
pub use range::{ClientBitString, ReadRangeResult};
pub use rustbac_bacnet_sc::BacnetScTransport;
pub use rustbac_core::services::acknowledge_alarm::{EventState, TimeStamp};
pub use rustbac_core::services::device_management::{DeviceCommunicationState, ReinitializeState};
pub use rustbac_datalink::bip::transport::{BroadcastDistributionEntry, ForeignDeviceTableEntry};
pub use value::ClientDataValue;
