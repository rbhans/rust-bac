//! Long-running async notification listener.
//!
//! Provides a notification listener that receives COV and event notifications
//! and dispatches them through an unbounded channel.

use crate::{ClientDataValue, CovNotification, CovPropertyValue, EventNotification};
use rustbac_core::apdu::{ApduType, ConfirmedRequestHeader, SimpleAck, UnconfirmedRequestHeader};
use rustbac_core::encoding::{reader::Reader, writer::Writer};
use rustbac_core::npdu::Npdu;
use rustbac_core::services::acknowledge_alarm::EventState;
use rustbac_core::services::cov_notification::{
    CovNotificationRequest, SERVICE_CONFIRMED_COV_NOTIFICATION,
    SERVICE_UNCONFIRMED_COV_NOTIFICATION,
};
use rustbac_core::services::event_notification::{
    EventNotificationRequest, SERVICE_CONFIRMED_EVENT_NOTIFICATION,
    SERVICE_UNCONFIRMED_EVENT_NOTIFICATION,
};
use rustbac_datalink::{DataLink, DataLinkAddress};
use std::sync::Arc;
use tokio::sync::mpsc;

/// A notification received from a BACnet device.
#[derive(Debug, Clone)]
pub enum Notification {
    Cov(CovNotification),
    Event(EventNotification),
}

/// A receiver for BACnet notifications dispatched by a listener loop.
pub struct NotificationListener {
    rx: mpsc::UnboundedReceiver<Notification>,
}

impl NotificationListener {
    /// Receive the next notification, waiting indefinitely.
    pub async fn recv(&mut self) -> Option<Notification> {
        self.rx.recv().await
    }
}

/// Create a notification listener and the async driver loop.
///
/// Returns `(listener, driver)` where `driver` is a future that must be
/// polled (e.g. via `tokio::spawn`) for notifications to be received.
/// The driver runs until the [`NotificationListener`] is dropped.
///
/// # Example
///
/// ```ignore
/// let (mut listener, driver) = create_notification_listener(datalink);
/// tokio::spawn(driver);
/// while let Some(notification) = listener.recv().await {
///     // handle notification
/// }
/// ```
pub fn create_notification_listener<D: DataLink + 'static>(
    datalink: Arc<D>,
) -> (NotificationListener, impl std::future::Future<Output = ()>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let driver = async move {
        let mut buf = [0u8; 1500];
        loop {
            let (n, source) = match datalink.recv(&mut buf).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            if let Some((notification, ack)) = parse_notification(&buf[..n], source) {
                if let Some(ack_bytes) = ack {
                    let _ = datalink.send(source, &ack_bytes).await;
                }
                if tx.send(notification).is_err() {
                    break; // receiver dropped
                }
            }
        }
    };

    (NotificationListener { rx }, driver)
}

fn parse_notification(
    frame: &[u8],
    source: DataLinkAddress,
) -> Option<(Notification, Option<Vec<u8>>)> {
    let apdu = extract_apdu(frame)?;
    let first = *apdu.first()?;
    let apdu_type = ApduType::from_u8(first >> 4)?;

    match apdu_type {
        ApduType::UnconfirmedRequest => {
            let mut r = Reader::new(apdu);
            let header = UnconfirmedRequestHeader::decode(&mut r).ok()?;
            match header.service_choice {
                SERVICE_UNCONFIRMED_COV_NOTIFICATION => {
                    let cov = CovNotificationRequest::decode_after_header(&mut r).ok()?;
                    let notification = build_cov_notification(source, false, cov)?;
                    Some((Notification::Cov(notification), None))
                }
                SERVICE_UNCONFIRMED_EVENT_NOTIFICATION => {
                    let evt = EventNotificationRequest::decode_after_header(&mut r).ok()?;
                    let notification = build_event_notification(source, false, evt)?;
                    Some((Notification::Event(notification), None))
                }
                _ => None,
            }
        }
        ApduType::ConfirmedRequest => {
            let mut r = Reader::new(apdu);
            let header = ConfirmedRequestHeader::decode(&mut r).ok()?;
            match header.service_choice {
                SERVICE_CONFIRMED_COV_NOTIFICATION => {
                    let cov = CovNotificationRequest::decode_after_header(&mut r).ok()?;
                    let notification = build_cov_notification(source, true, cov)?;
                    let ack =
                        build_simple_ack(header.invoke_id, SERVICE_CONFIRMED_COV_NOTIFICATION);
                    Some((Notification::Cov(notification), Some(ack)))
                }
                SERVICE_CONFIRMED_EVENT_NOTIFICATION => {
                    let evt = EventNotificationRequest::decode_after_header(&mut r).ok()?;
                    let notification = build_event_notification(source, true, evt)?;
                    let ack =
                        build_simple_ack(header.invoke_id, SERVICE_CONFIRMED_EVENT_NOTIFICATION);
                    Some((Notification::Event(notification), Some(ack)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn extract_apdu(frame: &[u8]) -> Option<&[u8]> {
    let mut r = Reader::new(frame);
    Npdu::decode(&mut r).ok()?;
    let remaining = r.remaining();
    if remaining == 0 {
        return None;
    }
    Some(&frame[frame.len() - remaining..])
}

fn build_simple_ack(invoke_id: u8, service_choice: u8) -> Vec<u8> {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    SimpleAck {
        invoke_id,
        service_choice,
    }
    .encode(&mut w)
    .unwrap();
    w.as_written().to_vec()
}

fn build_cov_notification(
    source: DataLinkAddress,
    confirmed: bool,
    cov: CovNotificationRequest<'_>,
) -> Option<CovNotification> {
    let values = cov
        .values
        .into_iter()
        .filter_map(|v| {
            Some(CovPropertyValue {
                property_id: v.property_id,
                array_index: v.array_index,
                value: into_client_value(v.value)?,
                priority: v.priority,
            })
        })
        .collect();

    Some(CovNotification {
        source,
        confirmed,
        subscriber_process_id: cov.subscriber_process_id,
        initiating_device_id: cov.initiating_device_id,
        monitored_object_id: cov.monitored_object_id,
        time_remaining_seconds: cov.time_remaining_seconds,
        values,
    })
}

fn build_event_notification(
    source: DataLinkAddress,
    confirmed: bool,
    evt: EventNotificationRequest<'_>,
) -> Option<EventNotification> {
    Some(EventNotification {
        source,
        confirmed,
        process_id: evt.process_id,
        initiating_device_id: evt.initiating_device_id,
        event_object_id: evt.event_object_id,
        timestamp: evt.timestamp,
        notification_class: evt.notification_class,
        priority: evt.priority,
        event_type: evt.event_type,
        message_text: evt.message_text.map(|s| s.to_string()),
        notify_type: evt.notify_type,
        ack_required: evt.ack_required,
        from_state_raw: evt.from_state,
        from_state: EventState::from_u32(evt.from_state),
        to_state_raw: evt.to_state,
        to_state: EventState::from_u32(evt.to_state),
    })
}

fn into_client_value(value: rustbac_core::types::DataValue<'_>) -> Option<ClientDataValue> {
    use rustbac_core::types::DataValue;
    Some(match value {
        DataValue::Null => ClientDataValue::Null,
        DataValue::Boolean(v) => ClientDataValue::Boolean(v),
        DataValue::Unsigned(v) => ClientDataValue::Unsigned(v),
        DataValue::Signed(v) => ClientDataValue::Signed(v),
        DataValue::Real(v) => ClientDataValue::Real(v),
        DataValue::Double(v) => ClientDataValue::Double(v),
        DataValue::OctetString(v) => ClientDataValue::OctetString(v.to_vec()),
        DataValue::CharacterString(v) => ClientDataValue::CharacterString(v.to_string()),
        DataValue::BitString(v) => ClientDataValue::BitString {
            unused_bits: v.unused_bits,
            data: v.data.to_vec(),
        },
        DataValue::Enumerated(v) => ClientDataValue::Enumerated(v),
        DataValue::Date(v) => ClientDataValue::Date(v),
        DataValue::Time(v) => ClientDataValue::Time(v),
        DataValue::ObjectId(v) => ClientDataValue::ObjectId(v),
        DataValue::Constructed { tag_num, values } => {
            let children: Vec<_> = values.into_iter().filter_map(into_client_value).collect();
            ClientDataValue::Constructed {
                tag_num,
                values: children,
            }
        }
    })
}
