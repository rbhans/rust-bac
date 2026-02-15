use crate::ClientDataValue;
use rustbac_core::types::{ObjectId, PropertyId};
use rustbac_datalink::DataLinkAddress;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CovPropertyValue {
    pub property_id: PropertyId,
    pub array_index: Option<u32>,
    pub value: ClientDataValue,
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CovNotification {
    pub source: DataLinkAddress,
    pub confirmed: bool,
    pub subscriber_process_id: u32,
    pub initiating_device_id: ObjectId,
    pub monitored_object_id: ObjectId,
    pub time_remaining_seconds: u32,
    pub values: Vec<CovPropertyValue>,
}
