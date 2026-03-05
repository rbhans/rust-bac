//! Integration tests for [`BacnetClient`] + [`SimulatedDevice`].
//!
//! Each test spins up an in-memory [`ChannelLink`] pair so that no real UDP
//! socket is needed. The client end is wrapped in a [`BacnetClient`] and the
//! server end drives a [`SimulatedDevice`]. Tests that exercise services the
//! simulator does not natively handle (ReadPropertyMultiple, DCC, segmented
//! responses, COV notifications) spin a small ad-hoc responder task on the
//! server link.

use rustbac_client::{
    BacnetClient, ClientDataValue, ClientError, Notification, SimulatedDevice,
};
use rustbac_core::{
    apdu::{ApduType, ComplexAckHeader, ConfirmedRequestHeader, SimpleAck},
    encoding::{
        primitives::{encode_app_real, encode_ctx_object_id, encode_ctx_unsigned},
        reader::Reader,
        tag::Tag,
        writer::Writer,
    },
    npdu::Npdu,
    services::{
        cov_notification::SERVICE_UNCONFIRMED_COV_NOTIFICATION,
        device_management::{DeviceCommunicationState, SERVICE_DEVICE_COMMUNICATION_CONTROL},
        read_property::SERVICE_READ_PROPERTY,
        read_property_multiple::SERVICE_READ_PROPERTY_MULTIPLE,
        subscribe_cov::SERVICE_SUBSCRIBE_COV,
        value_codec::encode_application_data_value,
    },
    types::{DataValue, ObjectId, ObjectType, PropertyId},
};
use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{mpsc, Mutex},
    time::timeout,
};

// ---------------------------------------------------------------------------
// In-memory bidirectional DataLink
// ---------------------------------------------------------------------------

/// One end of an in-memory channel link.
///
/// Messages sent on this link arrive at the paired [`ChannelLink`] and vice
/// versa. The source address reported on `recv` is always `PEER_ADDR`.
#[derive(Clone)]
struct ChannelLink {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
}

/// Synthetic "address" used as the peer address for in-memory links.
const CLIENT_ADDR: DataLinkAddress =
    DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1));
const SERVER_ADDR: DataLinkAddress =
    DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 2));

/// Creates a connected (client_link, server_link) pair.
///
/// Messages sent from client_link appear at server_link with `source = CLIENT_ADDR`,
/// and messages sent from server_link appear at client_link with `source = SERVER_ADDR`.
fn make_link_pair() -> (ChannelLink, ChannelLink) {
    let (client_tx, server_rx) = mpsc::channel::<Vec<u8>>(64);
    let (server_tx, client_rx) = mpsc::channel::<Vec<u8>>(64);

    let client_link = ChannelLink {
        tx: client_tx,
        rx: Arc::new(Mutex::new(client_rx)),
    };
    let server_link = ChannelLink {
        tx: server_tx,
        rx: Arc::new(Mutex::new(server_rx)),
    };
    (client_link, server_link)
}

impl DataLink for ChannelLink {
    async fn send(&self, _address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError> {
        // Ignore send errors — the receiver may have shut down already (e.g.
        // after the last segment was received). Silently dropping a message
        // mirrors the fire-and-forget semantics of UDP.
        let _ = self.tx.send(payload.to_vec()).await;
        Ok(())
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
        // Determine which address to report as the source based on which
        // channel this is: if we're the client link, messages come from the
        // server, and vice versa.
        let msg = self
            .rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(DataLinkError::InvalidFrame)?;
        if msg.len() > buf.len() {
            return Err(DataLinkError::FrameTooLarge);
        }
        buf[..msg.len()].copy_from_slice(&msg);
        Ok((msg.len(), SERVER_ADDR))
    }
}

// ---------------------------------------------------------------------------
// Helper: wrap bare APDU bytes in an NPDU header
// ---------------------------------------------------------------------------
fn with_npdu(apdu: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 512];
    let mut w = Writer::new(&mut out);
    Npdu::new(0).encode(&mut w).unwrap();
    w.write_all(apdu).unwrap();
    let len = w.as_written().len();
    out.truncate(len);
    out
}

// ---------------------------------------------------------------------------
// Helper: build a SimulatedDevice with an AnalogInput and AnalogOutput
// ---------------------------------------------------------------------------
async fn make_simulator(server_link: ChannelLink) -> SimulatedDevice<ChannelLink> {
    let sim = SimulatedDevice::new(100, server_link);

    // AnalogInput object 1 — PresentValue = 42.0
    let mut ai_props = HashMap::new();
    ai_props.insert(PropertyId::PresentValue, ClientDataValue::Real(42.0));
    ai_props.insert(
        PropertyId::ObjectName,
        ClientDataValue::CharacterString("AI-1".to_string()),
    );
    let ai_id = ObjectId::new(ObjectType::AnalogInput, 1);
    sim.add_object(ai_id, ai_props).await;

    // AnalogOutput object 1 — PresentValue = 0.0
    let mut ao_props = HashMap::new();
    ao_props.insert(PropertyId::PresentValue, ClientDataValue::Real(0.0));
    ao_props.insert(
        PropertyId::ObjectName,
        ClientDataValue::CharacterString("AO-1".to_string()),
    );
    let ao_id = ObjectId::new(ObjectType::AnalogOutput, 1);
    sim.add_object(ao_id, ao_props).await;

    sim
}

// ---------------------------------------------------------------------------
// Test 1: read_property — read PresentValue from AnalogInput
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_read_property() {
    let (client_link, server_link) = make_link_pair();
    let sim = make_simulator(server_link).await;

    // Run the simulator in the background.
    tokio::spawn(async move { sim.run().await });

    let client = BacnetClient::with_datalink(client_link)
        .with_response_timeout(Duration::from_secs(5));

    let object_id = ObjectId::new(ObjectType::AnalogInput, 1);
    let result = timeout(
        Duration::from_secs(5),
        client.read_property(SERVER_ADDR, object_id, PropertyId::PresentValue),
    )
    .await
    .expect("test timed out")
    .expect("read_property failed");

    assert_eq!(
        result,
        ClientDataValue::Real(42.0),
        "expected PresentValue = 42.0"
    );
}

// ---------------------------------------------------------------------------
// Test 2: write_property then read back
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_write_property() {
    let (client_link, server_link) = make_link_pair();
    let sim = make_simulator(server_link).await;
    tokio::spawn(async move { sim.run().await });

    let client = BacnetClient::with_datalink(client_link)
        .with_response_timeout(Duration::from_secs(5));

    let ao_id = ObjectId::new(ObjectType::AnalogOutput, 1);

    // Write PresentValue = 99.5
    timeout(
        Duration::from_secs(5),
        client.write_property(
            SERVER_ADDR,
            rustbac_core::services::write_property::WritePropertyRequest {
                object_id: ao_id,
                property_id: PropertyId::PresentValue,
                value: DataValue::Real(99.5),
                array_index: None,
                priority: None,
                invoke_id: 0, // will be overwritten by client
            },
        ),
    )
    .await
    .expect("test timed out")
    .expect("write_property failed");

    // Read back
    let result = timeout(
        Duration::from_secs(5),
        client.read_property(SERVER_ADDR, ao_id, PropertyId::PresentValue),
    )
    .await
    .expect("test timed out")
    .expect("read_property after write failed");

    assert_eq!(
        result,
        ClientDataValue::Real(99.5),
        "expected PresentValue = 99.5 after write"
    );
}

// ---------------------------------------------------------------------------
// Test 3: read_property_multiple — ObjectName + PresentValue in one call
//
// The SimulatedDevice does not handle ReadPropertyMultiple natively.  We
// therefore spin an ad-hoc responder that decodes the request and replies with
// a hand-crafted ComplexAck.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_read_property_multiple() {
    let (client_link, server_link) = make_link_pair();

    let ai_id = ObjectId::new(ObjectType::AnalogInput, 1);

    // Ad-hoc RPM responder — receives the client's request, builds a reply.
    let responder_link = server_link.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            let Ok((n, _src)) = responder_link.recv(&mut buf).await else {
                break;
            };
            let frame = &buf[..n];
            let mut r = Reader::new(frame);
            let Ok(_npdu) = Npdu::decode(&mut r) else {
                continue;
            };
            let Ok(first_byte) = r.peek_u8() else {
                continue;
            };
            if (first_byte >> 4) != (ApduType::ConfirmedRequest as u8) {
                continue;
            }
            let Ok(hdr) = ConfirmedRequestHeader::decode(&mut r) else {
                continue;
            };
            if hdr.service_choice != SERVICE_READ_PROPERTY_MULTIPLE {
                continue;
            }

            // Build ReadPropertyMultiple-Ack with ObjectName + PresentValue.
            let mut apdu_buf = [0u8; 256];
            let mut w = Writer::new(&mut apdu_buf);
            ComplexAckHeader {
                segmented: false,
                more_follows: false,
                invoke_id: hdr.invoke_id,
                sequence_number: None,
                proposed_window_size: None,
                service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
            }
            .encode(&mut w)
            .unwrap();

            // object-identifier [0]
            encode_ctx_unsigned(&mut w, 0, ai_id.raw()).unwrap();
            // list-of-results [1]
            Tag::Opening { tag_num: 1 }.encode(&mut w).unwrap();

            // ObjectName result: propertyIdentifier [2], readResult [4]
            encode_ctx_unsigned(&mut w, 2, PropertyId::ObjectName.to_u32()).unwrap();
            Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
            encode_application_data_value(&mut w, &DataValue::CharacterString("AI-1")).unwrap();
            Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

            // PresentValue result
            encode_ctx_unsigned(&mut w, 2, PropertyId::PresentValue.to_u32()).unwrap();
            Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
            encode_app_real(&mut w, 42.0).unwrap();
            Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

            Tag::Closing { tag_num: 1 }.encode(&mut w).unwrap();

            let apdu = w.as_written().to_vec();
            let frame = with_npdu(&apdu);
            let _ = responder_link.send(CLIENT_ADDR, &frame).await;
            break;
        }
    });

    let client = BacnetClient::with_datalink(client_link)
        .with_response_timeout(Duration::from_secs(5));

    let props = [PropertyId::ObjectName, PropertyId::PresentValue];
    let result = timeout(
        Duration::from_secs(5),
        client.read_property_multiple(SERVER_ADDR, ai_id, &props),
    )
    .await
    .expect("test timed out")
    .expect("read_property_multiple failed");

    assert_eq!(result.len(), 2, "expected two property results");

    let name_val = result
        .iter()
        .find(|(pid, _)| *pid == PropertyId::ObjectName)
        .map(|(_, v)| v.clone());
    let pv_val = result
        .iter()
        .find(|(pid, _)| *pid == PropertyId::PresentValue)
        .map(|(_, v)| v.clone());

    assert_eq!(
        name_val,
        Some(ClientDataValue::CharacterString("AI-1".to_string())),
        "ObjectName mismatch"
    );
    assert_eq!(
        pv_val,
        Some(ClientDataValue::Real(42.0)),
        "PresentValue mismatch"
    );
}

// ---------------------------------------------------------------------------
// Test 4: who_is — client broadcasts Who-Is; simulator replies with I-Am
//
// Because who_is() sends to the broadcast address the SimulatedDevice
// (reachable via the in-memory link) never actually receives that unicast
// broadcast. Instead we run a small forwarder: the responder task reads the
// Who-Is forwarded to it via a separate channel and sends the I-Am reply back
// to the client link so that who_is() can collect it.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_who_is_discovers_device() {
    // We need a custom ChannelLink whose `send` intercepts the broadcast
    // Who-Is and routes it to the SimulatedDevice, then routes the I-Am reply
    // back.  The easiest approach is to keep two pairs of channels.

    // client -> who_is_tx (captured by sim task)
    let (who_is_tx, mut who_is_rx) = mpsc::channel::<Vec<u8>>(8);
    // sim sends I-Am -> iam_tx -> client_rx
    let (iam_tx, iam_rx) = mpsc::channel::<Vec<u8>>(8);

    /// DataLink for the client: intercepts broadcasts and unicasts alike.
    #[derive(Clone)]
    struct WhoIsClientLink {
        /// Outbound channel (client -> sim).
        out: mpsc::Sender<Vec<u8>>,
        /// Inbound channel (sim -> client).
        inp: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    }
    impl DataLink for WhoIsClientLink {
        async fn send(
            &self,
            _addr: DataLinkAddress,
            payload: &[u8],
        ) -> Result<(), DataLinkError> {
            self.out
                .send(payload.to_vec())
                .await
                .map_err(|_| DataLinkError::InvalidFrame)
        }
        async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
            let msg = self
                .inp
                .lock()
                .await
                .recv()
                .await
                .ok_or(DataLinkError::InvalidFrame)?;
            if msg.len() > buf.len() {
                return Err(DataLinkError::FrameTooLarge);
            }
            buf[..msg.len()].copy_from_slice(&msg);
            Ok((msg.len(), SERVER_ADDR))
        }
    }

    // Override server's send so it goes into iam_tx.
    let iam_tx_clone = iam_tx.clone();
    let server_link2 = {
        #[derive(Clone)]
        struct SimLink {
            out: mpsc::Sender<Vec<u8>>,
            inp: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
        }
        impl DataLink for SimLink {
            async fn send(
                &self,
                _addr: DataLinkAddress,
                payload: &[u8],
            ) -> Result<(), DataLinkError> {
                self.out
                    .send(payload.to_vec())
                    .await
                    .map_err(|_| DataLinkError::InvalidFrame)
            }
            async fn recv(
                &self,
                buf: &mut [u8],
            ) -> Result<(usize, DataLinkAddress), DataLinkError> {
                let msg = self
                    .inp
                    .lock()
                    .await
                    .recv()
                    .await
                    .ok_or(DataLinkError::InvalidFrame)?;
                if msg.len() > buf.len() {
                    return Err(DataLinkError::FrameTooLarge);
                }
                buf[..msg.len()].copy_from_slice(&msg);
                Ok((msg.len(), CLIENT_ADDR))
            }
        }

        let (sim_rx_tx, sim_rx_rx) = mpsc::channel::<Vec<u8>>(8);
        // Wire: who_is messages from client -> sim_rx_tx; forwarder task below.
        let _forwarder_handle = {
            let sim_inp = sim_rx_tx.clone();
            tokio::spawn(async move {
                while let Some(msg) = who_is_rx.recv().await {
                    let _ = sim_inp.send(msg).await;
                }
            })
        };

        SimLink {
            out: iam_tx_clone,
            inp: Arc::new(Mutex::new(sim_rx_rx)),
        }
    };

    let sim = SimulatedDevice::new(100, server_link2);
    tokio::spawn(async move { sim.run().await });

    let client_link = WhoIsClientLink {
        out: who_is_tx,
        inp: Arc::new(Mutex::new(iam_rx)),
    };

    let client = BacnetClient::with_datalink(client_link)
        .with_response_timeout(Duration::from_secs(5));

    let devices = timeout(
        Duration::from_secs(5),
        client.who_is(None, Duration::from_millis(500)),
    )
    .await
    .expect("test timed out")
    .expect("who_is failed");

    let device_id = ObjectId::new(ObjectType::Device, 100);
    let found = devices.iter().any(|d| d.device_id == Some(device_id));
    assert!(
        found,
        "expected device 100 in who_is results, got: {devices:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: subscribe_cov_then_notify
//
// The SimulatedDevice does not implement COV subscription bookkeeping or
// notifications.  We therefore:
//   a) Use an ad-hoc responder that sends a SimpleAck for the SubscribeCov
//      request (so the client is satisfied).
//   b) After the subscription succeeds, the responder sends an unconfirmed
//      COV notification to the client's listener link.
//   c) The test asserts the notification is received via a
//      `create_notification_listener`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_subscribe_cov_then_notify() {
    use rustbac_client::create_notification_listener;

    let (client_link, server_link) = make_link_pair();

    let ai_id = ObjectId::new(ObjectType::AnalogInput, 1);

    // Ad-hoc responder:
    // 1. Sends SimpleAck for SubscribeCov
    // 2. Sends an unconfirmed COV notification
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];

        // Step 1: receive SubscribeCov request and ack it.
        let (n, _src) = server_link.recv(&mut buf).await.expect("recv");
        let frame = &buf[..n];
        let mut r = Reader::new(frame);
        Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(
            hdr.service_choice, SERVICE_SUBSCRIBE_COV,
            "expected SubscribeCov"
        );

        // Send SimpleAck.
        let mut ack_buf = [0u8; 64];
        let mut w = Writer::new(&mut ack_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        SimpleAck {
            invoke_id: hdr.invoke_id,
            service_choice: SERVICE_SUBSCRIBE_COV,
        }
        .encode(&mut w)
        .unwrap();
        let ack = w.as_written().to_vec();
        server_link.send(CLIENT_ADDR, &ack).await.unwrap();

        // Step 2: send an unconfirmed COV notification with PresentValue=77.0.
        let mut cov_buf = [0u8; 256];
        let mut w = Writer::new(&mut cov_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        // UnconfirmedRequest header
        let unconf_hdr_byte: u8 = (ApduType::UnconfirmedRequest as u8) << 4;
        w.write_u8(unconf_hdr_byte).unwrap();
        w.write_u8(SERVICE_UNCONFIRMED_COV_NOTIFICATION).unwrap();
        // COV payload: subscriber-process-id [0], initiating-device-id [1],
        // monitored-object-id [2], time-remaining [3], list-of-values [4]
        encode_ctx_unsigned(&mut w, 0, 1).unwrap(); // subscriber-process-id
        encode_ctx_object_id(&mut w, 1, ObjectId::new(ObjectType::Device, 100).raw()).unwrap();
        encode_ctx_object_id(&mut w, 2, ai_id.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 3, 30).unwrap(); // time-remaining
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        // property-value: property-identifier [0], value [2]
        encode_ctx_unsigned(&mut w, 0, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();
        encode_application_data_value(&mut w, &DataValue::Real(77.0)).unwrap();
        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        let cov_frame = w.as_written().to_vec();
        server_link.send(CLIENT_ADDR, &cov_frame).await.unwrap();
    });

    // Subscribe COV via the client.
    let client = Arc::new(
        BacnetClient::with_datalink(client_link.clone())
            .with_response_timeout(Duration::from_secs(5)),
    );

    timeout(
        Duration::from_secs(5),
        client.subscribe_cov(
            SERVER_ADDR,
            rustbac_core::services::subscribe_cov::SubscribeCovRequest {
                subscriber_process_id: 1,
                monitored_object_id: ai_id,
                issue_confirmed_notifications: Some(false),
                lifetime_seconds: Some(30),
                invoke_id: 0,
            },
        ),
    )
    .await
    .expect("test timed out")
    .expect("subscribe_cov failed");

    // After the subscribe_cov call has returned, the responder will send the
    // COV notification.  Set up a listener on the same datalink.
    let (mut listener, driver) = create_notification_listener(Arc::new(client_link));
    tokio::spawn(driver);

    let notification = timeout(Duration::from_secs(5), listener.recv())
        .await
        .expect("notification timed out")
        .expect("listener channel closed");

    let cov = match notification {
        Notification::Cov(c) => c,
        other => panic!("expected COV notification, got {other:?}"),
    };

    assert_eq!(
        cov.monitored_object_id, ai_id,
        "monitored object mismatch"
    );
    let pv = cov
        .values
        .iter()
        .find(|v| v.property_id == PropertyId::PresentValue)
        .expect("no PresentValue in COV notification");
    assert_eq!(
        pv.value,
        ClientDataValue::Real(77.0),
        "COV PresentValue mismatch"
    );
}

// ---------------------------------------------------------------------------
// Test 6: device_communication_control — assert SimpleAck
//
// The SimulatedDevice ignores DCC.  We use an ad-hoc responder.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_device_communication_control() {
    let (client_link, server_link) = make_link_pair();

    // Ad-hoc DCC responder.
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        let (n, _src) = server_link.recv(&mut buf).await.expect("recv");
        let frame = &buf[..n];
        let mut r = Reader::new(frame);
        Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(
            hdr.service_choice, SERVICE_DEVICE_COMMUNICATION_CONTROL,
            "expected DCC"
        );

        let mut ack_buf = [0u8; 64];
        let mut w = Writer::new(&mut ack_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        SimpleAck {
            invoke_id: hdr.invoke_id,
            service_choice: SERVICE_DEVICE_COMMUNICATION_CONTROL,
        }
        .encode(&mut w)
        .unwrap();
        let ack = w.as_written().to_vec();
        server_link.send(CLIENT_ADDR, &ack).await.unwrap();
    });

    let client = BacnetClient::with_datalink(client_link)
        .with_response_timeout(Duration::from_secs(5));

    let result = timeout(
        Duration::from_secs(5),
        client.device_communication_control(
            SERVER_ADDR,
            None,
            DeviceCommunicationState::Enable,
            None,
        ),
    )
    .await
    .expect("test timed out");

    assert!(result.is_ok(), "device_communication_control failed: {result:?}");
}

// ---------------------------------------------------------------------------
// Test 7: segmented_read
//
// The simulator stores properties and responds to ReadProperty requests. The
// client's `collect_complex_ack_payload` assembles segmented responses.  We
// use an ad-hoc responder that sends a two-segment ComplexAck for a large
// CharacterString property, simulating a device with a small max-APDU window.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_segmented_read() {
    let (client_link, server_link) = make_link_pair();

    // Build a large value to send back in two segments.
    let large_string: String = "X".repeat(300);
    let large_string_clone = large_string.clone();

    tokio::spawn(async move {
        let mut buf = [0u8; 1500];

        // Receive the ReadProperty request.
        let (n, _src) = server_link.recv(&mut buf).await.expect("recv ReadProperty");
        let frame = &buf[..n];
        let mut r = Reader::new(frame);
        Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_READ_PROPERTY, "expected RPR");
        let invoke_id = hdr.invoke_id;

        // Encode the full ReadPropertyAck payload (without the APDU header).
        let object_id = ObjectId::new(ObjectType::AnalogInput, 1);
        let mut payload_buf = vec![0u8; 4096];
        let payload_len = {
            let mut w = Writer::new(&mut payload_buf);
            encode_ctx_unsigned(&mut w, 0, object_id.raw()).unwrap();
            encode_ctx_unsigned(&mut w, 1, PropertyId::Description.to_u32()).unwrap();
            Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
            encode_application_data_value(
                &mut w,
                &DataValue::CharacterString(&large_string_clone),
            )
            .unwrap();
            Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
            w.as_written().len()
        };
        let payload = &payload_buf[..payload_len];

        // Split into two segments of roughly equal size.
        let mid = payload_len / 2;
        let seg0 = &payload[..mid];
        let seg1 = &payload[mid..];

        // Segment 0 — more_follows=true.
        {
            let mut f = vec![0u8; 1024];
            let mut w = Writer::new(&mut f);
            Npdu::new(0).encode(&mut w).unwrap();
            ComplexAckHeader {
                segmented: true,
                more_follows: true,
                invoke_id,
                sequence_number: Some(0),
                proposed_window_size: Some(1),
                service_choice: SERVICE_READ_PROPERTY,
            }
            .encode(&mut w)
            .unwrap();
            w.write_all(seg0).unwrap();
            let len = w.as_written().len();
            f.truncate(len);
            server_link.send(CLIENT_ADDR, &f).await.unwrap();
        }

        // Receive the SegmentAck from the client for segment 0.
        let (n, _src) = server_link.recv(&mut buf).await.expect("segment ack recv");
        let seg_ack_frame = &buf[..n];
        let mut r = Reader::new(seg_ack_frame);
        Npdu::decode(&mut r).unwrap();
        let first = r.peek_u8().unwrap();
        assert_eq!(
            first >> 4,
            ApduType::SegmentAck as u8,
            "expected SegmentAck after segment 0"
        );

        // Segment 1 — more_follows=false.
        {
            let mut f = vec![0u8; 1024];
            let mut w = Writer::new(&mut f);
            Npdu::new(0).encode(&mut w).unwrap();
            ComplexAckHeader {
                segmented: true,
                more_follows: false,
                invoke_id,
                sequence_number: Some(1),
                proposed_window_size: Some(1),
                service_choice: SERVICE_READ_PROPERTY,
            }
            .encode(&mut w)
            .unwrap();
            w.write_all(seg1).unwrap();
            let len = w.as_written().len();
            f.truncate(len);
            server_link.send(CLIENT_ADDR, &f).await.unwrap();
        }
        // The client will send a final SegmentAck for segment 1 — we don't
        // need to receive it, since ChannelLink::send() is fire-and-forget.
    });

    let client = BacnetClient::with_datalink(client_link)
        .with_response_timeout(Duration::from_secs(5));

    let object_id = ObjectId::new(ObjectType::AnalogInput, 1);
    let result = timeout(
        Duration::from_secs(5),
        client.read_property(SERVER_ADDR, object_id, PropertyId::Description),
    )
    .await
    .expect("test timed out")
    .expect("segmented read_property failed");

    assert_eq!(
        result,
        ClientDataValue::CharacterString(large_string),
        "segmented response value mismatch"
    );
}

// ---------------------------------------------------------------------------
// Test 8: timeout_on_no_response
//
// Point the client at a ChannelLink whose receiver is immediately closed
// (no sender side), so every `recv` returns immediately with an error.
// The client's internal deadline should fire and return ClientError::Timeout.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_timeout_on_no_response() {
    // A DataLink that accepts sends but never delivers any reply.
    #[derive(Clone)]
    struct SilentLink {
        _sink: mpsc::Sender<Vec<u8>>,
    }
    impl DataLink for SilentLink {
        async fn send(
            &self,
            _addr: DataLinkAddress,
            _payload: &[u8],
        ) -> Result<(), DataLinkError> {
            Ok(())
        }
        async fn recv(
            &self,
            _buf: &mut [u8],
        ) -> Result<(usize, DataLinkAddress), DataLinkError> {
            // Park forever — the client's own deadline will fire.
            tokio::time::sleep(Duration::from_secs(600)).await;
            Err(DataLinkError::InvalidFrame)
        }
    }

    let (sink, _) = mpsc::channel::<Vec<u8>>(1);
    let link = SilentLink { _sink: sink };

    // Short timeout so the test finishes quickly.
    let client = BacnetClient::with_datalink(link)
        .with_response_timeout(Duration::from_millis(200));

    let object_id = ObjectId::new(ObjectType::AnalogInput, 1);
    let result = timeout(
        Duration::from_secs(5),
        client.read_property(SERVER_ADDR, object_id, PropertyId::PresentValue),
    )
    .await
    .expect("outer timeout — client did not respect its own deadline");

    assert!(
        matches!(result, Err(ClientError::Timeout)),
        "expected ClientError::Timeout, got {result:?}"
    );
}
