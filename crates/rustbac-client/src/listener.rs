//! Long-running async notification listener.
//!
//! Provides a notification listener that receives COV and event notifications
//! and dispatches them through a bounded channel.

use crate::{ClientDataValue, CovNotification, CovPropertyValue, EventNotification};
use rustbac_core::apdu::{
    abort_reason, AbortPdu, ApduType, ConfirmedRequestHeader, SimpleAck, UnconfirmedRequestHeader,
};
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

/// Default capacity of the bounded channel used by [`create_notification_listener`].
///
/// When this many notifications are queued without being consumed the driver loop drops
/// new arrivals rather than growing the queue without bound.
pub const DEFAULT_NOTIFICATION_CHANNEL_CAPACITY: usize = 256;

/// A notification received from a BACnet device — either a COV or an event notification.
#[derive(Debug, Clone)]
pub enum Notification {
    /// A change-of-value notification (confirmed or unconfirmed SubscribeCOV / SubscribeCOVProperty).
    Cov(CovNotification),
    /// An event notification (confirmed or unconfirmed EventNotification service).
    Event(EventNotification),
}

/// Consumer half of a BACnet notification channel.
///
/// Produced by [`create_notification_listener`] or
/// [`create_notification_listener_with_capacity`]. The channel is closed and `recv`
/// returns `None` once the driver future finishes (i.e. when this receiver is dropped).
pub struct NotificationListener {
    rx: mpsc::Receiver<Notification>,
}

impl NotificationListener {
    /// Wait for and return the next notification. Returns `None` when the driver has stopped.
    pub async fn recv(&mut self) -> Option<Notification> {
        self.rx.recv().await
    }
}

/// Create a notification listener backed by a channel with [`DEFAULT_NOTIFICATION_CHANNEL_CAPACITY`].
///
/// Returns `(listener, driver)` where `driver` is a future that must be
/// polled (e.g. via `tokio::spawn`) for notifications to be received.
/// The driver runs until the [`NotificationListener`] is dropped. When the channel is
/// full, incoming notifications are silently discarded to bound memory usage.
/// Confirmed notifications are automatically acknowledged; segmented confirmed
/// notifications are rejected with an Abort PDU (segmentation not supported).
pub fn create_notification_listener<D: DataLink + 'static>(
    datalink: Arc<D>,
) -> (NotificationListener, impl std::future::Future<Output = ()>) {
    create_notification_listener_with_capacity(datalink, DEFAULT_NOTIFICATION_CHANNEL_CAPACITY)
}

/// Like [`create_notification_listener`] but with an explicit channel `capacity`.
///
/// `capacity` is clamped to a minimum of 1. Prefer
/// [`create_notification_listener`] unless you need a non-default buffer size.
pub fn create_notification_listener_with_capacity<D: DataLink + 'static>(
    datalink: Arc<D>,
    capacity: usize,
) -> (NotificationListener, impl std::future::Future<Output = ()>) {
    let (tx, rx) = mpsc::channel(capacity.max(1));
    let driver = async move {
        let mut buf = [0u8; 1500];
        loop {
            let (n, source) = match datalink.recv(&mut buf).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            match parse_notification(&buf[..n], source) {
                ParseResult::None => {}
                ParseResult::Abort(ack_bytes) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("segmented notification aborted — segmentation not supported");
                    let _ = datalink.send(source, &ack_bytes).await;
                }
                ParseResult::Notification(notification, ack) => {
                    if let Some(ack_bytes) = ack {
                        let _ = datalink.send(source, &ack_bytes).await;
                    }
                    // Drop notifications when the consumer is slow rather than
                    // growing the queue without bound. Break only when the
                    // receiver has been dropped; a full channel just discards
                    // this notification.
                    match tx.try_send(notification) {
                        Ok(()) => {}
                        Err(_) if tx.is_closed() => break, // receiver dropped
                        Err(_) => {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("notification channel full — dropping notification");
                        }
                    }
                }
            }
        }
    };

    (NotificationListener { rx }, driver)
}

enum ParseResult {
    None,
    /// Segmented request we cannot handle — send an Abort, emit no notification.
    Abort(Vec<u8>),
    /// Parsed notification and optional ack to send back.
    Notification(Notification, Option<Vec<u8>>),
}

fn parse_notification(frame: &[u8], source: DataLinkAddress) -> ParseResult {
    let apdu = match extract_apdu(frame) {
        Some(a) => a,
        None => return ParseResult::None,
    };
    let first = match apdu.first() {
        Some(&b) => b,
        None => return ParseResult::None,
    };
    let apdu_type = match ApduType::from_u8(first >> 4) {
        Some(t) => t,
        None => return ParseResult::None,
    };

    match apdu_type {
        ApduType::UnconfirmedRequest => {
            let mut r = Reader::new(apdu);
            let header = match UnconfirmedRequestHeader::decode(&mut r) {
                Ok(h) => h,
                Err(_) => return ParseResult::None,
            };
            match header.service_choice {
                SERVICE_UNCONFIRMED_COV_NOTIFICATION => {
                    let cov = match CovNotificationRequest::decode_after_header(&mut r) {
                        Ok(c) => c,
                        Err(_) => return ParseResult::None,
                    };
                    match build_cov_notification(source, false, cov) {
                        Some(n) => ParseResult::Notification(Notification::Cov(n), None),
                        None => ParseResult::None,
                    }
                }
                SERVICE_UNCONFIRMED_EVENT_NOTIFICATION => {
                    let evt = match EventNotificationRequest::decode_after_header(&mut r) {
                        Ok(e) => e,
                        Err(_) => return ParseResult::None,
                    };
                    match build_event_notification(source, false, evt) {
                        Some(n) => ParseResult::Notification(Notification::Event(n), None),
                        None => ParseResult::None,
                    }
                }
                _ => ParseResult::None,
            }
        }
        ApduType::ConfirmedRequest => {
            let mut r = Reader::new(apdu);
            let header = match ConfirmedRequestHeader::decode(&mut r) {
                Ok(h) => h,
                Err(_) => return ParseResult::None,
            };

            // Segmented confirmed notifications are not supported. Send an
            // Abort so the remote device knows and doesn't keep retrying.
            if header.segmented {
                return ParseResult::Abort(build_abort(header.invoke_id));
            }

            match header.service_choice {
                SERVICE_CONFIRMED_COV_NOTIFICATION => {
                    let cov = match CovNotificationRequest::decode_after_header(&mut r) {
                        Ok(c) => c,
                        Err(_) => return ParseResult::None,
                    };
                    match build_cov_notification(source, true, cov) {
                        Some(n) => {
                            let ack = build_simple_ack(
                                header.invoke_id,
                                SERVICE_CONFIRMED_COV_NOTIFICATION,
                            );
                            ParseResult::Notification(Notification::Cov(n), Some(ack))
                        }
                        None => ParseResult::None,
                    }
                }
                SERVICE_CONFIRMED_EVENT_NOTIFICATION => {
                    let evt = match EventNotificationRequest::decode_after_header(&mut r) {
                        Ok(e) => e,
                        Err(_) => return ParseResult::None,
                    };
                    match build_event_notification(source, true, evt) {
                        Some(n) => {
                            let ack = build_simple_ack(
                                header.invoke_id,
                                SERVICE_CONFIRMED_EVENT_NOTIFICATION,
                            );
                            ParseResult::Notification(Notification::Event(n), Some(ack))
                        }
                        None => ParseResult::None,
                    }
                }
                _ => ParseResult::None,
            }
        }
        _ => ParseResult::None,
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

fn build_abort(invoke_id: u8) -> Vec<u8> {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    AbortPdu {
        server: false,
        invoke_id,
        reason: abort_reason::SEGMENTATION_NOT_SUPPORTED,
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
