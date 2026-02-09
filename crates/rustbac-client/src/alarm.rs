use crate::ClientBitString;
use rustbac_core::services::acknowledge_alarm::{EventState, TimeStamp};
use rustbac_core::types::ObjectId;
use rustbac_datalink::DataLinkAddress;

#[derive(Debug, Clone, PartialEq)]
pub struct AlarmSummaryItem {
    pub object_id: ObjectId,
    pub alarm_state_raw: u32,
    pub alarm_state: Option<EventState>,
    pub acknowledged_transitions: ClientBitString,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnrollmentSummaryItem {
    pub object_id: ObjectId,
    pub event_type: u32,
    pub event_state_raw: u32,
    pub event_state: Option<EventState>,
    pub priority: u32,
    pub notification_class: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventInformationItem {
    pub object_id: ObjectId,
    pub event_state_raw: u32,
    pub event_state: Option<EventState>,
    pub acknowledged_transitions: ClientBitString,
    pub notify_type: u32,
    pub event_enable: ClientBitString,
    pub event_priorities: [u32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventInformationResult {
    pub summaries: Vec<EventInformationItem>,
    pub more_events: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventNotification {
    pub source: DataLinkAddress,
    pub confirmed: bool,
    pub process_id: u32,
    pub initiating_device_id: ObjectId,
    pub event_object_id: ObjectId,
    pub timestamp: TimeStamp,
    pub notification_class: u32,
    pub priority: u32,
    pub event_type: u32,
    pub message_text: Option<String>,
    pub notify_type: u32,
    pub ack_required: Option<bool>,
    pub from_state_raw: u32,
    pub from_state: Option<EventState>,
    pub to_state_raw: u32,
    pub to_state: Option<EventState>,
}
