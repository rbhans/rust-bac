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
/// COV subscriptions with renewal and polling failover.
pub mod cov_manager;
/// Device and object discovery (Who-Is / I-Am / Who-Has).
pub mod discovery;
/// Client-level error type.
pub mod error;
/// Atomic file read/write operations.
pub mod file;
/// Long-running async notification listener.
pub mod listener;
/// Point type inference for BACnet objects.
pub mod point;
/// ReadRange results and related types.
pub mod range;
/// Schedule and Calendar convenience helpers.
pub mod schedule;
/// Lightweight simulated BACnet device.
pub mod simulator;
/// Per-device request throttling utility.
pub mod throttle;
/// Owned application-data values for client-side use.
pub mod value;
/// Device discovery walk — reads all objects and their properties.
pub mod walk;

pub use alarm::{
    AlarmSummaryItem, EnrollmentSummaryItem, EventInformationItem, EventInformationResult,
    EventNotification,
};
pub use client::{BacnetClient, ForeignDeviceRenewal};
pub use cov::{CovNotification, CovPropertyValue};
pub use cov_manager::{
    CovManager, CovManagerBuilder, CovSubscriptionSpec, CovUpdate, UpdateSource,
};
pub use discovery::{DiscoveredDevice, DiscoveredObject};
pub use error::ClientError;
pub use file::{AtomicReadFileResult, AtomicWriteFileResult};
pub use listener::{create_notification_listener, Notification, NotificationListener};
pub use point::{PointClassification, PointDirection, PointKind};
pub use range::{ClientBitString, ReadRangeResult};
pub use rustbac_bacnet_sc::BacnetScTransport;
pub use rustbac_core::services::acknowledge_alarm::{EventState, TimeStamp};
pub use rustbac_core::services::device_management::{DeviceCommunicationState, ReinitializeState};
pub use rustbac_datalink::bip::transport::{BroadcastDistributionEntry, ForeignDeviceTableEntry};
pub use schedule::{CalendarEntry, DateRange, TimeValue};
pub use simulator::SimulatedDevice;
pub use throttle::DeviceThrottle;
pub use value::ClientDataValue;
pub use walk::{DeviceWalkResult, ObjectSummary};

// Internal helpers used by simulator module.
use rustbac_core::encoding::{primitives::decode_unsigned, reader::Reader, tag::Tag};
use rustbac_core::types::ObjectId;

fn decode_ctx_unsigned(r: &mut Reader<'_>) -> Result<u32, ClientError> {
    match Tag::decode(r)? {
        Tag::Context { len, .. } => Ok(decode_unsigned(r, len as usize)?),
        _ => Err(ClientError::UnsupportedResponse),
    }
}

fn decode_ctx_object_id(r: &mut Reader<'_>) -> Result<ObjectId, ClientError> {
    Ok(ObjectId::from_raw(decode_ctx_unsigned(r)?))
}

fn data_value_to_client(value: rustbac_core::types::DataValue<'_>) -> ClientDataValue {
    match value {
        rustbac_core::types::DataValue::Null => ClientDataValue::Null,
        rustbac_core::types::DataValue::Boolean(v) => ClientDataValue::Boolean(v),
        rustbac_core::types::DataValue::Unsigned(v) => ClientDataValue::Unsigned(v),
        rustbac_core::types::DataValue::Signed(v) => ClientDataValue::Signed(v),
        rustbac_core::types::DataValue::Real(v) => ClientDataValue::Real(v),
        rustbac_core::types::DataValue::Double(v) => ClientDataValue::Double(v),
        rustbac_core::types::DataValue::OctetString(v) => ClientDataValue::OctetString(v.to_vec()),
        rustbac_core::types::DataValue::CharacterString(v) => {
            ClientDataValue::CharacterString(v.to_string())
        }
        rustbac_core::types::DataValue::BitString(v) => ClientDataValue::BitString {
            unused_bits: v.unused_bits,
            data: v.data.to_vec(),
        },
        rustbac_core::types::DataValue::Enumerated(v) => ClientDataValue::Enumerated(v),
        rustbac_core::types::DataValue::Date(v) => ClientDataValue::Date(v),
        rustbac_core::types::DataValue::Time(v) => ClientDataValue::Time(v),
        rustbac_core::types::DataValue::ObjectId(v) => ClientDataValue::ObjectId(v),
        rustbac_core::types::DataValue::Constructed { tag_num, values } => {
            ClientDataValue::Constructed {
                tag_num,
                values: values.into_iter().map(data_value_to_client).collect(),
            }
        }
    }
}
