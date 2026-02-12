use crate::{
    AlarmSummaryItem, AtomicReadFileResult, AtomicWriteFileResult, ClientBitString,
    ClientDataValue, ClientError, CovNotification, CovPropertyValue, DiscoveredDevice,
    DiscoveredObject, EnrollmentSummaryItem, EventInformationItem, EventInformationResult,
    EventNotification, ReadRangeResult,
};
use rustbac_bacnet_sc::BacnetScTransport;
use rustbac_core::apdu::{
    AbortPdu, ApduType, BacnetError, ComplexAckHeader, ConfirmedRequestHeader, RejectPdu,
    SegmentAck, SimpleAck, UnconfirmedRequestHeader,
};
use rustbac_core::encoding::{reader::Reader, writer::Writer};
use rustbac_core::npdu::Npdu;
use rustbac_core::services::acknowledge_alarm::{
    AcknowledgeAlarmRequest, SERVICE_ACKNOWLEDGE_ALARM,
};
use rustbac_core::services::alarm_summary::{
    AlarmSummaryItem as CoreAlarmSummaryItem, GetAlarmSummaryAck, GetAlarmSummaryRequest,
    SERVICE_GET_ALARM_SUMMARY,
};
use rustbac_core::services::atomic_read_file::{
    AtomicReadFileAck, AtomicReadFileAckAccess, AtomicReadFileRequest, SERVICE_ATOMIC_READ_FILE,
};
use rustbac_core::services::atomic_write_file::{
    AtomicWriteFileAck, AtomicWriteFileRequest, SERVICE_ATOMIC_WRITE_FILE,
};
use rustbac_core::services::cov_notification::{
    CovNotificationRequest, SERVICE_CONFIRMED_COV_NOTIFICATION,
    SERVICE_UNCONFIRMED_COV_NOTIFICATION,
};
use rustbac_core::services::device_management::{
    DeviceCommunicationControlRequest, DeviceCommunicationState, ReinitializeDeviceRequest,
    ReinitializeState, SERVICE_DEVICE_COMMUNICATION_CONTROL, SERVICE_REINITIALIZE_DEVICE,
};
use rustbac_core::services::enrollment_summary::{
    EnrollmentSummaryItem as CoreEnrollmentSummaryItem, GetEnrollmentSummaryAck,
    GetEnrollmentSummaryRequest, SERVICE_GET_ENROLLMENT_SUMMARY,
};
use rustbac_core::services::event_information::{
    EventSummaryItem as CoreEventSummaryItem, GetEventInformationAck, GetEventInformationRequest,
    SERVICE_GET_EVENT_INFORMATION,
};
use rustbac_core::services::event_notification::{
    EventNotificationRequest, SERVICE_CONFIRMED_EVENT_NOTIFICATION,
    SERVICE_UNCONFIRMED_EVENT_NOTIFICATION,
};
use rustbac_core::services::i_am::{IAmRequest, SERVICE_I_AM};
use rustbac_core::services::list_element::{
    AddListElementRequest, RemoveListElementRequest, SERVICE_ADD_LIST_ELEMENT,
    SERVICE_REMOVE_LIST_ELEMENT,
};
use rustbac_core::services::object_management::{
    CreateObjectAck, CreateObjectRequest, DeleteObjectRequest, SERVICE_CREATE_OBJECT,
    SERVICE_DELETE_OBJECT,
};
use rustbac_core::services::private_transfer::{
    ConfirmedPrivateTransferAck as PrivateTransferAck, ConfirmedPrivateTransferRequest,
    SERVICE_CONFIRMED_PRIVATE_TRANSFER,
};
use rustbac_core::services::read_property::{
    ReadPropertyAck, ReadPropertyRequest, SERVICE_READ_PROPERTY,
};
use rustbac_core::services::read_property_multiple::{
    PropertyReference, ReadAccessSpecification, ReadPropertyMultipleAck,
    ReadPropertyMultipleRequest, SERVICE_READ_PROPERTY_MULTIPLE,
};
use rustbac_core::services::read_range::{ReadRangeAck, ReadRangeRequest, SERVICE_READ_RANGE};
use rustbac_core::services::subscribe_cov::{SubscribeCovRequest, SERVICE_SUBSCRIBE_COV};
use rustbac_core::services::subscribe_cov_property::{
    SubscribeCovPropertyRequest, SERVICE_SUBSCRIBE_COV_PROPERTY,
};
use rustbac_core::services::time_synchronization::TimeSynchronizationRequest;
use rustbac_core::services::who_has::{IHaveRequest, WhoHasObject, WhoHasRequest, SERVICE_I_HAVE};
use rustbac_core::services::who_is::WhoIsRequest;
use rustbac_core::services::write_property::{WritePropertyRequest, SERVICE_WRITE_PROPERTY};
use rustbac_core::services::write_property_multiple::{
    PropertyWriteSpec, WriteAccessSpecification, WritePropertyMultipleRequest,
    SERVICE_WRITE_PROPERTY_MULTIPLE,
};
use rustbac_core::types::{DataValue, Date, ErrorClass, ErrorCode, ObjectId, PropertyId, Time};
use rustbac_core::EncodeError;
use rustbac_datalink::bip::transport::{
    BacnetIpTransport, BroadcastDistributionEntry, ForeignDeviceTableEntry,
};
use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Instant};

const MIN_SEGMENT_DATA_LEN: usize = 32;
const MAX_COMPLEX_ACK_REASSEMBLY_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct BacnetClient<D: DataLink> {
    datalink: D,
    invoke_id: Mutex<u8>,
    request_io_lock: Mutex<()>,
    response_timeout: Duration,
    segmented_request_window_size: u8,
    segmented_request_retries: u8,
    segment_ack_timeout: Duration,
}

#[derive(Debug)]
pub struct ForeignDeviceRenewal {
    task: JoinHandle<()>,
}

impl ForeignDeviceRenewal {
    pub fn stop(self) {
        self.task.abort();
    }
}

impl Drop for ForeignDeviceRenewal {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl BacnetClient<BacnetIpTransport> {
    pub async fn new() -> Result<Self, ClientError> {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
        let datalink = BacnetIpTransport::bind(bind_addr).await?;
        Ok(Self {
            datalink,
            invoke_id: Mutex::new(1),
            request_io_lock: Mutex::new(()),
            response_timeout: Duration::from_secs(3),
            segmented_request_window_size: 1,
            segmented_request_retries: 2,
            segment_ack_timeout: Duration::from_millis(500),
        })
    }

    pub async fn new_foreign(bbmd_addr: SocketAddr, ttl_seconds: u16) -> Result<Self, ClientError> {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
        let datalink = BacnetIpTransport::bind_foreign(bind_addr, bbmd_addr).await?;
        datalink.register_foreign_device(ttl_seconds).await?;
        Ok(Self {
            datalink,
            invoke_id: Mutex::new(1),
            request_io_lock: Mutex::new(()),
            response_timeout: Duration::from_secs(3),
            segmented_request_window_size: 1,
            segmented_request_retries: 2,
            segment_ack_timeout: Duration::from_millis(500),
        })
    }

    pub async fn register_foreign_device(&self, ttl_seconds: u16) -> Result<(), ClientError> {
        self.datalink.register_foreign_device(ttl_seconds).await?;
        Ok(())
    }

    pub async fn read_broadcast_distribution_table(
        &self,
    ) -> Result<Vec<BroadcastDistributionEntry>, ClientError> {
        self.datalink
            .read_broadcast_distribution_table()
            .await
            .map_err(ClientError::from)
    }

    pub async fn write_broadcast_distribution_table(
        &self,
        entries: &[BroadcastDistributionEntry],
    ) -> Result<(), ClientError> {
        self.datalink
            .write_broadcast_distribution_table(entries)
            .await?;
        Ok(())
    }

    pub async fn read_foreign_device_table(
        &self,
    ) -> Result<Vec<ForeignDeviceTableEntry>, ClientError> {
        self.datalink
            .read_foreign_device_table()
            .await
            .map_err(ClientError::from)
    }

    pub async fn delete_foreign_device_table_entry(
        &self,
        address: SocketAddrV4,
    ) -> Result<(), ClientError> {
        self.datalink
            .delete_foreign_device_table_entry(address)
            .await?;
        Ok(())
    }

    pub fn start_foreign_device_renewal(
        &self,
        ttl_seconds: u16,
    ) -> Result<ForeignDeviceRenewal, ClientError> {
        if ttl_seconds == 0 {
            return Err(EncodeError::InvalidLength.into());
        }

        let datalink = self.datalink.clone();
        let refresh_seconds = u64::from(ttl_seconds).saturating_mul(3) / 4;
        let interval = Duration::from_secs(refresh_seconds.max(1));
        let task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if let Err(err) = datalink.register_foreign_device_no_wait(ttl_seconds).await {
                    log::warn!("foreign device renewal send failed: {err}");
                }
            }
        });
        Ok(ForeignDeviceRenewal { task })
    }
}

impl BacnetClient<BacnetScTransport> {
    pub async fn new_sc(endpoint: impl Into<String>) -> Result<Self, ClientError> {
        let datalink = BacnetScTransport::connect(endpoint).await?;
        Ok(Self::with_datalink(datalink))
    }
}

impl<D: DataLink> BacnetClient<D> {
    pub fn with_datalink(datalink: D) -> Self {
        Self {
            datalink,
            invoke_id: Mutex::new(1),
            request_io_lock: Mutex::new(()),
            response_timeout: Duration::from_secs(3),
            segmented_request_window_size: 1,
            segmented_request_retries: 2,
            segment_ack_timeout: Duration::from_millis(500),
        }
    }

    pub fn with_response_timeout(mut self, timeout: Duration) -> Self {
        self.response_timeout = timeout;
        self
    }

    pub fn with_segmented_request_window_size(mut self, window_size: u8) -> Self {
        self.segmented_request_window_size = window_size.max(1);
        self
    }

    pub fn with_segmented_request_retries(mut self, retries: u8) -> Self {
        self.segmented_request_retries = retries;
        self
    }

    pub fn with_segment_ack_timeout(mut self, timeout: Duration) -> Self {
        self.segment_ack_timeout = timeout.max(Duration::from_millis(1));
        self
    }

    async fn next_invoke_id(&self) -> u8 {
        let mut lock = self.invoke_id.lock().await;
        let id = *lock;
        *lock = lock.wrapping_add(1);
        if *lock == 0 {
            *lock = 1;
        }
        id
    }

    async fn send_segment_ack(
        &self,
        address: DataLinkAddress,
        invoke_id: u8,
        sequence_number: u8,
        window_size: u8,
    ) -> Result<(), ClientError> {
        let mut tx = [0u8; 64];
        let mut w = Writer::new(&mut tx);
        Npdu::new(0).encode(&mut w)?;
        SegmentAck {
            negative_ack: false,
            sent_by_server: false,
            invoke_id,
            sequence_number,
            actual_window_size: window_size,
        }
        .encode(&mut w)?;
        self.datalink.send(address, w.as_written()).await?;
        Ok(())
    }

    async fn recv_ignoring_invalid_frame(
        &self,
        buf: &mut [u8],
        deadline: Instant,
    ) -> Result<(usize, DataLinkAddress), ClientError> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ClientError::Timeout);
            }

            match timeout(remaining, self.datalink.recv(buf)).await {
                Err(_) => return Err(ClientError::Timeout),
                Ok(Err(DataLinkError::InvalidFrame)) => continue,
                Ok(Err(e)) => return Err(e.into()),
                Ok(Ok(v)) => return Ok(v),
            }
        }
    }

    async fn send_simple_ack(
        &self,
        address: DataLinkAddress,
        invoke_id: u8,
        service_choice: u8,
    ) -> Result<(), ClientError> {
        let mut tx = [0u8; 64];
        let mut w = Writer::new(&mut tx);
        Npdu::new(0).encode(&mut w)?;
        SimpleAck {
            invoke_id,
            service_choice,
        }
        .encode(&mut w)?;
        self.datalink.send(address, w.as_written()).await?;
        Ok(())
    }

    fn encode_with_growth<F>(&self, mut encode: F) -> Result<Vec<u8>, ClientError>
    where
        F: FnMut(&mut Writer<'_>) -> Result<(), EncodeError>,
    {
        for size in [512usize, 1024, 2048, 4096, 8192, 16_384, 32_768, 65_536] {
            let mut buf = vec![0u8; size];
            let mut w = Writer::new(&mut buf);
            match encode(&mut w) {
                Ok(()) => {
                    let written_len = w.as_written().len();
                    buf.truncate(written_len);
                    return Ok(buf);
                }
                Err(EncodeError::BufferTooSmall) => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(ClientError::SegmentedRequestTooLarge)
    }

    const fn max_apdu_octets(max_apdu_code: u8) -> usize {
        match max_apdu_code & 0x0f {
            0 => 50,
            1 => 128,
            2 => 206,
            3 => 480,
            4 => 1024,
            5 => 1476,
            _ => 480,
        }
    }

    async fn await_segment_ack(
        &self,
        address: DataLinkAddress,
        invoke_id: u8,
        service_choice: u8,
        expected_sequence: u8,
        deadline: Instant,
    ) -> Result<SegmentAck, ClientError> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ClientError::Timeout);
            }

            let mut rx = [0u8; 1500];
            let recv = timeout(remaining, self.datalink.recv(&mut rx)).await;
            let (n, src) = match recv {
                Err(_) => return Err(ClientError::Timeout),
                Ok(Err(DataLinkError::InvalidFrame)) => continue,
                Ok(Err(e)) => return Err(e.into()),
                Ok(Ok(v)) => v,
            };
            if src != address {
                continue;
            }

            let Ok(apdu) = extract_apdu(&rx[..n]) else {
                continue;
            };
            let first = *apdu.first().ok_or(ClientError::UnsupportedResponse)?;
            match ApduType::from_u8(first >> 4) {
                Some(ApduType::SegmentAck) => {
                    let mut r = Reader::new(apdu);
                    let ack = SegmentAck::decode(&mut r)?;
                    if ack.invoke_id != invoke_id || !ack.sent_by_server {
                        continue;
                    }
                    if ack.negative_ack {
                        return Err(ClientError::SegmentNegativeAck {
                            sequence_number: ack.sequence_number,
                        });
                    }
                    if ack.sequence_number == expected_sequence {
                        return Ok(ack);
                    }
                }
                Some(ApduType::Error) => {
                    let mut r = Reader::new(apdu);
                    let err = BacnetError::decode(&mut r)?;
                    if err.invoke_id == invoke_id && err.service_choice == service_choice {
                        return Err(remote_service_error(err));
                    }
                }
                Some(ApduType::Reject) => {
                    let mut r = Reader::new(apdu);
                    let rej = RejectPdu::decode(&mut r)?;
                    if rej.invoke_id == invoke_id {
                        return Err(ClientError::RemoteReject { reason: rej.reason });
                    }
                }
                Some(ApduType::Abort) => {
                    let mut r = Reader::new(apdu);
                    let abort = AbortPdu::decode(&mut r)?;
                    if abort.invoke_id == invoke_id {
                        return Err(ClientError::RemoteAbort {
                            reason: abort.reason,
                            server: abort.server,
                        });
                    }
                }
                _ => continue,
            }
        }
    }

    async fn send_confirmed_request(
        &self,
        address: DataLinkAddress,
        frame: &[u8],
        deadline: Instant,
    ) -> Result<(), ClientError> {
        let mut pr = Reader::new(frame);
        let _npdu = Npdu::decode(&mut pr)?;
        let npdu_len = frame.len() - pr.remaining();
        let npdu_bytes = &frame[..npdu_len];
        let apdu = &frame[npdu_len..];

        let mut ar = Reader::new(apdu);
        let header = ConfirmedRequestHeader::decode(&mut ar)?;
        let service_payload = ar.read_exact(ar.remaining())?;

        let segment_data_len = Self::max_apdu_octets(header.max_apdu)
            .saturating_sub(5)
            .max(MIN_SEGMENT_DATA_LEN);
        let segment_count = service_payload.len().div_ceil(segment_data_len);

        if segment_count <= 1 {
            self.datalink.send(address, frame).await?;
            return Ok(());
        }

        if segment_count > usize::from(u8::MAX) + 1 {
            return Err(ClientError::SegmentedRequestTooLarge);
        }

        let configured_window_size = self.segmented_request_window_size.max(1);
        let mut window_size = configured_window_size;
        let mut peer_window_ceiling = configured_window_size;
        let mut batch_start = 0usize;
        while batch_start < segment_count {
            let batch_end = (batch_start + usize::from(window_size)).min(segment_count);
            let expected_sequence = (batch_end - 1) as u8;

            let mut frames = Vec::with_capacity(batch_end - batch_start);
            for segment_index in batch_start..batch_end {
                let seq = segment_index as u8;
                let more_follows = segment_index + 1 < segment_count;
                let start = segment_index * segment_data_len;
                let end = ((segment_index + 1) * segment_data_len).min(service_payload.len());
                let segment = &service_payload[start..end];

                let seg_header = ConfirmedRequestHeader {
                    segmented: true,
                    more_follows,
                    segmented_response_accepted: header.segmented_response_accepted,
                    max_segments: header.max_segments,
                    max_apdu: header.max_apdu,
                    invoke_id: header.invoke_id,
                    sequence_number: Some(seq),
                    proposed_window_size: Some(window_size),
                    service_choice: header.service_choice,
                };

                let mut tx = vec![0u8; npdu_bytes.len() + 16 + segment.len()];
                let written_len = {
                    let mut w = Writer::new(&mut tx);
                    w.write_all(npdu_bytes)?;
                    seg_header.encode(&mut w)?;
                    w.write_all(segment)?;
                    w.as_written().len()
                };
                tx.truncate(written_len);
                frames.push(tx);
            }

            let mut retries_remaining = self.segmented_request_retries;
            loop {
                for frame in &frames {
                    self.datalink.send(address, frame).await?;
                }

                if batch_end == segment_count {
                    break;
                }

                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(ClientError::Timeout);
                }
                let ack_wait_deadline = Instant::now() + remaining.min(self.segment_ack_timeout);
                match self
                    .await_segment_ack(
                        address,
                        header.invoke_id,
                        header.service_choice,
                        expected_sequence,
                        ack_wait_deadline,
                    )
                    .await
                {
                    Ok(ack) => {
                        peer_window_ceiling =
                            peer_window_ceiling.min(ack.actual_window_size.max(1));
                        window_size = window_size
                            .saturating_add(1)
                            .min(configured_window_size)
                            .min(peer_window_ceiling)
                            .max(1);
                        break;
                    }
                    Err(ClientError::Timeout | ClientError::SegmentNegativeAck { .. })
                        if retries_remaining > 0 =>
                    {
                        retries_remaining -= 1;
                        window_size = window_size.saturating_div(2).max(1);
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }

            batch_start = batch_end;
        }

        Ok(())
    }

    async fn collect_complex_ack_payload(
        &self,
        address: DataLinkAddress,
        invoke_id: u8,
        service_choice: u8,
        first_header: ComplexAckHeader,
        first_payload: &[u8],
        deadline: Instant,
    ) -> Result<Vec<u8>, ClientError> {
        let mut payload = first_payload.to_vec();
        if payload.len() > MAX_COMPLEX_ACK_REASSEMBLY_BYTES {
            return Err(ClientError::ResponseTooLarge {
                limit: MAX_COMPLEX_ACK_REASSEMBLY_BYTES,
            });
        }
        if !first_header.segmented {
            return Ok(payload);
        }

        let mut last_seq = first_header
            .sequence_number
            .ok_or(ClientError::UnsupportedResponse)?;
        let mut window_size = first_header.proposed_window_size.unwrap_or(1);
        self.send_segment_ack(address, invoke_id, last_seq, window_size)
            .await?;
        let mut more_follows = first_header.more_follows;

        while more_follows {
            let mut rx = [0u8; 1500];
            let (n, src) = self.recv_ignoring_invalid_frame(&mut rx, deadline).await?;
            if src != address {
                continue;
            }

            let Ok(apdu) = extract_apdu(&rx[..n]) else {
                continue;
            };
            let first = *apdu.first().ok_or(ClientError::UnsupportedResponse)?;
            match ApduType::from_u8(first >> 4) {
                Some(ApduType::ComplexAck) => {
                    let mut r = Reader::new(apdu);
                    let seg = ComplexAckHeader::decode(&mut r)?;
                    if seg.invoke_id != invoke_id || seg.service_choice != service_choice {
                        continue;
                    }
                    if !seg.segmented {
                        return Err(ClientError::UnsupportedResponse);
                    }
                    let seq = seg
                        .sequence_number
                        .ok_or(ClientError::UnsupportedResponse)?;
                    if seq == last_seq {
                        // Duplicate segment: acknowledge again and continue waiting.
                        self.send_segment_ack(address, invoke_id, last_seq, window_size)
                            .await?;
                        continue;
                    }
                    if seq != last_seq.wrapping_add(1) {
                        continue;
                    }

                    let seg_payload = r.read_exact(r.remaining())?;
                    if payload.len().saturating_add(seg_payload.len())
                        > MAX_COMPLEX_ACK_REASSEMBLY_BYTES
                    {
                        return Err(ClientError::ResponseTooLarge {
                            limit: MAX_COMPLEX_ACK_REASSEMBLY_BYTES,
                        });
                    }
                    payload.extend_from_slice(seg_payload);

                    last_seq = seq;
                    more_follows = seg.more_follows;
                    window_size = seg.proposed_window_size.unwrap_or(window_size);
                    self.send_segment_ack(address, invoke_id, last_seq, window_size)
                        .await?;
                }
                Some(ApduType::Error) => {
                    let mut r = Reader::new(apdu);
                    let err = BacnetError::decode(&mut r)?;
                    if err.invoke_id == invoke_id && err.service_choice == service_choice {
                        return Err(remote_service_error(err));
                    }
                }
                Some(ApduType::Reject) => {
                    let mut r = Reader::new(apdu);
                    let rej = RejectPdu::decode(&mut r)?;
                    if rej.invoke_id == invoke_id {
                        return Err(ClientError::RemoteReject { reason: rej.reason });
                    }
                }
                Some(ApduType::Abort) => {
                    let mut r = Reader::new(apdu);
                    let abort = AbortPdu::decode(&mut r)?;
                    if abort.invoke_id == invoke_id {
                        return Err(ClientError::RemoteAbort {
                            reason: abort.reason,
                            server: abort.server,
                        });
                    }
                }
                _ => continue,
            }
        }

        Ok(payload)
    }

    pub async fn who_is(
        &self,
        range: Option<(u32, u32)>,
        wait: Duration,
    ) -> Result<Vec<DiscoveredDevice>, ClientError> {
        let _io_lock = self.request_io_lock.lock().await;
        let req = match range {
            Some((low, high)) => WhoIsRequest {
                low_limit: Some(low),
                high_limit: Some(high),
            },
            None => WhoIsRequest::global(),
        };

        let mut tx = [0u8; 128];
        let mut w = Writer::new(&mut tx);
        Npdu::new(0).encode(&mut w)?;
        req.encode(&mut w)?;

        self.datalink
            .send(
                DataLinkAddress::local_broadcast(DataLinkAddress::BACNET_IP_DEFAULT_PORT),
                w.as_written(),
            )
            .await?;

        let mut devices = Vec::new();
        let mut seen = HashSet::new();
        let deadline = tokio::time::Instant::now() + wait;

        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let mut rx = [0u8; 1500];
            let recv = timeout(remaining, self.datalink.recv(&mut rx)).await;
            match recv {
                Ok(Ok((n, src))) => {
                    let Ok(apdu) = extract_apdu(&rx[..n]) else {
                        continue;
                    };
                    let mut r = Reader::new(apdu);
                    let Ok(unconfirmed) = UnconfirmedRequestHeader::decode(&mut r) else {
                        continue;
                    };
                    if unconfirmed.service_choice != SERVICE_I_AM {
                        continue;
                    }
                    let Ok(i_am) = IAmRequest::decode_after_header(&mut r) else {
                        continue;
                    };
                    if seen.insert(src) {
                        devices.push(DiscoveredDevice {
                            address: src,
                            device_id: Some(i_am.device_id),
                        });
                    }
                }
                Ok(Err(DataLinkError::InvalidFrame)) => continue,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            }
        }

        Ok(devices)
    }

    pub async fn who_has_object_id(
        &self,
        range: Option<(u32, u32)>,
        object_id: ObjectId,
        wait: Duration,
    ) -> Result<Vec<DiscoveredObject>, ClientError> {
        let req = WhoHasRequest {
            low_limit: range.map(|(low, _)| low),
            high_limit: range.map(|(_, high)| high),
            object: WhoHasObject::ObjectId(object_id),
        };
        self.who_has(req, wait).await
    }

    pub async fn who_has_object_name(
        &self,
        range: Option<(u32, u32)>,
        object_name: &str,
        wait: Duration,
    ) -> Result<Vec<DiscoveredObject>, ClientError> {
        let req = WhoHasRequest {
            low_limit: range.map(|(low, _)| low),
            high_limit: range.map(|(_, high)| high),
            object: WhoHasObject::ObjectName(object_name),
        };
        self.who_has(req, wait).await
    }

    async fn who_has(
        &self,
        request: WhoHasRequest<'_>,
        wait: Duration,
    ) -> Result<Vec<DiscoveredObject>, ClientError> {
        let _io_lock = self.request_io_lock.lock().await;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.datalink
            .send(
                DataLinkAddress::local_broadcast(DataLinkAddress::BACNET_IP_DEFAULT_PORT),
                &tx,
            )
            .await?;

        let mut objects = Vec::new();
        let mut seen = HashSet::new();
        let deadline = tokio::time::Instant::now() + wait;

        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let mut rx = [0u8; 1500];
            let recv = timeout(remaining, self.datalink.recv(&mut rx)).await;
            match recv {
                Ok(Ok((n, src))) => {
                    let Ok(apdu) = extract_apdu(&rx[..n]) else {
                        continue;
                    };
                    let mut r = Reader::new(apdu);
                    let Ok(unconfirmed) = UnconfirmedRequestHeader::decode(&mut r) else {
                        continue;
                    };
                    if unconfirmed.service_choice != SERVICE_I_HAVE {
                        continue;
                    }
                    let Ok(i_have) = IHaveRequest::decode_after_header(&mut r) else {
                        continue;
                    };
                    if !seen.insert((src, i_have.object_id.raw())) {
                        continue;
                    }
                    objects.push(DiscoveredObject {
                        address: src,
                        device_id: i_have.device_id,
                        object_id: i_have.object_id,
                        object_name: i_have.object_name.to_string(),
                    });
                }
                Ok(Err(DataLinkError::InvalidFrame)) => continue,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            }
        }

        Ok(objects)
    }

    pub async fn device_communication_control(
        &self,
        address: DataLinkAddress,
        time_duration_seconds: Option<u16>,
        enable_disable: DeviceCommunicationState,
        password: Option<&str>,
    ) -> Result<(), ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = DeviceCommunicationControlRequest {
            time_duration_seconds,
            enable_disable,
            password,
            invoke_id,
        };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_DEVICE_COMMUNICATION_CONTROL,
            self.response_timeout,
        )
        .await
    }

    pub async fn reinitialize_device(
        &self,
        address: DataLinkAddress,
        state: ReinitializeState,
        password: Option<&str>,
    ) -> Result<(), ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = ReinitializeDeviceRequest {
            state,
            password,
            invoke_id,
        };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_REINITIALIZE_DEVICE,
            self.response_timeout,
        )
        .await
    }

    pub async fn time_synchronize(
        &self,
        address: DataLinkAddress,
        date: Date,
        time: Time,
        utc: bool,
    ) -> Result<(), ClientError> {
        let request = if utc {
            TimeSynchronizationRequest::utc(date, time)
        } else {
            TimeSynchronizationRequest::local(date, time)
        };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.datalink.send(address, &tx).await?;
        Ok(())
    }

    pub async fn create_object_by_type(
        &self,
        address: DataLinkAddress,
        object_type: rustbac_core::types::ObjectType,
    ) -> Result<ObjectId, ClientError> {
        self.create_object(address, CreateObjectRequest::by_type(object_type, 0))
            .await
    }

    pub async fn create_object(
        &self,
        address: DataLinkAddress,
        mut request: CreateObjectRequest,
    ) -> Result<ObjectId, ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_CREATE_OBJECT,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = CreateObjectAck::decode_after_header(&mut pr)?;
        Ok(parsed.object_id)
    }

    pub async fn delete_object(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
    ) -> Result<(), ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = DeleteObjectRequest {
            object_id,
            invoke_id,
        };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_DELETE_OBJECT,
            self.response_timeout,
        )
        .await
    }

    pub async fn add_list_element(
        &self,
        address: DataLinkAddress,
        mut request: AddListElementRequest<'_>,
    ) -> Result<(), ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_ADD_LIST_ELEMENT,
            self.response_timeout,
        )
        .await
    }

    pub async fn remove_list_element(
        &self,
        address: DataLinkAddress,
        mut request: RemoveListElementRequest<'_>,
    ) -> Result<(), ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_REMOVE_LIST_ELEMENT,
            self.response_timeout,
        )
        .await
    }

    async fn await_simple_ack_or_error(
        &self,
        address: DataLinkAddress,
        tx: &[u8],
        invoke_id: u8,
        service_choice: u8,
        timeout_window: Duration,
    ) -> Result<(), ClientError> {
        let _io_lock = self.request_io_lock.lock().await;
        let deadline = tokio::time::Instant::now() + timeout_window;
        self.send_confirmed_request(address, tx, deadline).await?;
        while tokio::time::Instant::now() < deadline {
            let mut rx = [0u8; 1500];
            let (n, src) = self.recv_ignoring_invalid_frame(&mut rx, deadline).await?;
            if src != address {
                continue;
            }
            let apdu = extract_apdu(&rx[..n])?;
            let first = *apdu.first().ok_or(ClientError::UnsupportedResponse)?;
            match ApduType::from_u8(first >> 4) {
                Some(ApduType::SimpleAck) => {
                    let mut r = Reader::new(apdu);
                    let ack = SimpleAck::decode(&mut r)?;
                    if ack.invoke_id == invoke_id && ack.service_choice == service_choice {
                        return Ok(());
                    }
                }
                Some(ApduType::Error) => {
                    let mut r = Reader::new(apdu);
                    let err = BacnetError::decode(&mut r)?;
                    if err.invoke_id == invoke_id && err.service_choice == service_choice {
                        return Err(remote_service_error(err));
                    }
                }
                Some(ApduType::Reject) => {
                    let mut r = Reader::new(apdu);
                    let rej = RejectPdu::decode(&mut r)?;
                    if rej.invoke_id == invoke_id {
                        return Err(ClientError::RemoteReject { reason: rej.reason });
                    }
                }
                Some(ApduType::Abort) => {
                    let mut r = Reader::new(apdu);
                    let abort = AbortPdu::decode(&mut r)?;
                    if abort.invoke_id == invoke_id {
                        return Err(ClientError::RemoteAbort {
                            reason: abort.reason,
                            server: abort.server,
                        });
                    }
                }
                _ => continue,
            }
        }
        Err(ClientError::Timeout)
    }

    async fn await_complex_ack_payload_or_error(
        &self,
        address: DataLinkAddress,
        tx: &[u8],
        invoke_id: u8,
        service_choice: u8,
        timeout_window: Duration,
    ) -> Result<Vec<u8>, ClientError> {
        let _io_lock = self.request_io_lock.lock().await;
        let deadline = tokio::time::Instant::now() + timeout_window;
        self.send_confirmed_request(address, tx, deadline).await?;
        while tokio::time::Instant::now() < deadline {
            let mut rx = [0u8; 1500];
            let (n, src) = self.recv_ignoring_invalid_frame(&mut rx, deadline).await?;
            if src != address {
                continue;
            }

            let apdu = extract_apdu(&rx[..n])?;
            let first = *apdu.first().ok_or(ClientError::UnsupportedResponse)?;
            match ApduType::from_u8(first >> 4) {
                Some(ApduType::ComplexAck) => {
                    let mut r = Reader::new(apdu);
                    let ack = ComplexAckHeader::decode(&mut r)?;
                    if ack.invoke_id != invoke_id || ack.service_choice != service_choice {
                        continue;
                    }
                    return self
                        .collect_complex_ack_payload(
                            address,
                            invoke_id,
                            service_choice,
                            ack,
                            r.read_exact(r.remaining())?,
                            deadline,
                        )
                        .await;
                }
                Some(ApduType::Error) => {
                    let mut r = Reader::new(apdu);
                    let err = BacnetError::decode(&mut r)?;
                    if err.invoke_id == invoke_id && err.service_choice == service_choice {
                        return Err(remote_service_error(err));
                    }
                }
                Some(ApduType::Reject) => {
                    let mut r = Reader::new(apdu);
                    let rej = RejectPdu::decode(&mut r)?;
                    if rej.invoke_id == invoke_id {
                        return Err(ClientError::RemoteReject { reason: rej.reason });
                    }
                }
                Some(ApduType::Abort) => {
                    let mut r = Reader::new(apdu);
                    let abort = AbortPdu::decode(&mut r)?;
                    if abort.invoke_id == invoke_id {
                        return Err(ClientError::RemoteAbort {
                            reason: abort.reason,
                            server: abort.server,
                        });
                    }
                }
                _ => continue,
            }
        }
        Err(ClientError::Timeout)
    }

    pub async fn get_alarm_summary(
        &self,
        address: DataLinkAddress,
    ) -> Result<Vec<AlarmSummaryItem>, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = GetAlarmSummaryRequest { invoke_id };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_GET_ALARM_SUMMARY,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = GetAlarmSummaryAck::decode_after_header(&mut pr)?;
        Ok(into_client_alarm_summary(parsed.summaries))
    }

    pub async fn get_enrollment_summary(
        &self,
        address: DataLinkAddress,
    ) -> Result<Vec<EnrollmentSummaryItem>, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = GetEnrollmentSummaryRequest { invoke_id };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_GET_ENROLLMENT_SUMMARY,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = GetEnrollmentSummaryAck::decode_after_header(&mut pr)?;
        Ok(into_client_enrollment_summary(parsed.summaries))
    }

    pub async fn get_event_information(
        &self,
        address: DataLinkAddress,
        last_received_object_id: Option<ObjectId>,
    ) -> Result<EventInformationResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = GetEventInformationRequest {
            last_received_object_id,
            invoke_id,
        };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_GET_EVENT_INFORMATION,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = GetEventInformationAck::decode_after_header(&mut pr)?;
        Ok(EventInformationResult {
            summaries: into_client_event_information(parsed.summaries),
            more_events: parsed.more_events,
        })
    }

    pub async fn acknowledge_alarm(
        &self,
        address: DataLinkAddress,
        mut request: AcknowledgeAlarmRequest<'_>,
    ) -> Result<(), ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_ACKNOWLEDGE_ALARM,
            self.response_timeout,
        )
        .await
    }

    pub async fn atomic_read_file_stream(
        &self,
        address: DataLinkAddress,
        file_object_id: ObjectId,
        file_start_position: i32,
        requested_octet_count: u32,
    ) -> Result<AtomicReadFileResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = AtomicReadFileRequest::stream(
            file_object_id,
            file_start_position,
            requested_octet_count,
            invoke_id,
        );
        self.atomic_read_file(address, request).await
    }

    pub async fn atomic_read_file_record(
        &self,
        address: DataLinkAddress,
        file_object_id: ObjectId,
        file_start_record: i32,
        requested_record_count: u32,
    ) -> Result<AtomicReadFileResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = AtomicReadFileRequest::record(
            file_object_id,
            file_start_record,
            requested_record_count,
            invoke_id,
        );
        self.atomic_read_file(address, request).await
    }

    async fn atomic_read_file(
        &self,
        address: DataLinkAddress,
        request: AtomicReadFileRequest,
    ) -> Result<AtomicReadFileResult, ClientError> {
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_ATOMIC_READ_FILE,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = AtomicReadFileAck::decode_after_header(&mut pr)?;
        Ok(into_client_atomic_read_result(parsed))
    }

    pub async fn atomic_write_file_stream(
        &self,
        address: DataLinkAddress,
        file_object_id: ObjectId,
        file_start_position: i32,
        file_data: &[u8],
    ) -> Result<AtomicWriteFileResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = AtomicWriteFileRequest::stream(
            file_object_id,
            file_start_position,
            file_data,
            invoke_id,
        );
        self.atomic_write_file(address, request).await
    }

    pub async fn atomic_write_file_record(
        &self,
        address: DataLinkAddress,
        file_object_id: ObjectId,
        file_start_record: i32,
        file_record_data: &[&[u8]],
    ) -> Result<AtomicWriteFileResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let request = AtomicWriteFileRequest::record(
            file_object_id,
            file_start_record,
            file_record_data,
            invoke_id,
        );
        self.atomic_write_file(address, request).await
    }

    async fn atomic_write_file(
        &self,
        address: DataLinkAddress,
        request: AtomicWriteFileRequest<'_>,
    ) -> Result<AtomicWriteFileResult, ClientError> {
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_ATOMIC_WRITE_FILE,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = AtomicWriteFileAck::decode_after_header(&mut pr)?;
        Ok(into_client_atomic_write_result(parsed))
    }

    pub async fn subscribe_cov(
        &self,
        address: DataLinkAddress,
        mut request: SubscribeCovRequest,
    ) -> Result<(), ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_SUBSCRIBE_COV,
            self.response_timeout,
        )
        .await
    }

    pub async fn cancel_cov_subscription(
        &self,
        address: DataLinkAddress,
        subscriber_process_id: u32,
        monitored_object_id: ObjectId,
    ) -> Result<(), ClientError> {
        self.subscribe_cov(
            address,
            SubscribeCovRequest::cancel(subscriber_process_id, monitored_object_id, 0),
        )
        .await
    }

    pub async fn subscribe_cov_property(
        &self,
        address: DataLinkAddress,
        mut request: SubscribeCovPropertyRequest,
    ) -> Result<(), ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_SUBSCRIBE_COV_PROPERTY,
            self.response_timeout,
        )
        .await
    }

    pub async fn cancel_cov_property_subscription(
        &self,
        address: DataLinkAddress,
        subscriber_process_id: u32,
        monitored_object_id: ObjectId,
        monitored_property_id: PropertyId,
        monitored_property_array_index: Option<u32>,
    ) -> Result<(), ClientError> {
        self.subscribe_cov_property(
            address,
            SubscribeCovPropertyRequest::cancel(
                subscriber_process_id,
                monitored_object_id,
                monitored_property_id,
                monitored_property_array_index,
                0,
            ),
        )
        .await
    }

    pub async fn read_range_by_position(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        reference_index: i32,
        count: i16,
    ) -> Result<ReadRangeResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let req = ReadRangeRequest::by_position(
            object_id,
            property_id,
            array_index,
            reference_index,
            count,
            invoke_id,
        );
        self.read_range_with_request(address, req).await
    }

    pub async fn read_range_by_sequence_number(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        reference_sequence: u32,
        count: i16,
    ) -> Result<ReadRangeResult, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let req = ReadRangeRequest::by_sequence_number(
            object_id,
            property_id,
            array_index,
            reference_sequence,
            count,
            invoke_id,
        );
        self.read_range_with_request(address, req).await
    }

    pub async fn read_range_by_time(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        at: (Date, Time),
        count: i16,
    ) -> Result<ReadRangeResult, ClientError> {
        let (date, time) = at;
        let invoke_id = self.next_invoke_id().await;
        let req = ReadRangeRequest::by_time(
            object_id,
            property_id,
            array_index,
            date,
            time,
            count,
            invoke_id,
        );
        self.read_range_with_request(address, req).await
    }

    async fn read_range_with_request(
        &self,
        address: DataLinkAddress,
        req: ReadRangeRequest,
    ) -> Result<ReadRangeResult, ClientError> {
        let invoke_id = req.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            req.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_READ_RANGE,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = ReadRangeAck::decode_after_header(&mut pr)?;
        into_client_read_range(parsed)
    }

    pub async fn recv_cov_notification(
        &self,
        wait: Duration,
    ) -> Result<Option<CovNotification>, ClientError> {
        let _io_lock = self.request_io_lock.lock().await;
        let deadline = tokio::time::Instant::now() + wait;

        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let mut rx = [0u8; 1500];
            let recv = timeout(remaining, self.datalink.recv(&mut rx)).await;
            let (n, source) = match recv {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            };

            let apdu = extract_apdu(&rx[..n])?;
            let first = *apdu.first().ok_or(ClientError::UnsupportedResponse)?;
            match ApduType::from_u8(first >> 4) {
                Some(ApduType::UnconfirmedRequest) => {
                    let mut r = Reader::new(apdu);
                    let header = UnconfirmedRequestHeader::decode(&mut r)?;
                    if header.service_choice != SERVICE_UNCONFIRMED_COV_NOTIFICATION {
                        continue;
                    }
                    let cov = CovNotificationRequest::decode_after_header(&mut r)?;
                    return Ok(Some(into_client_cov_notification(source, false, cov)?));
                }
                Some(ApduType::ConfirmedRequest) => {
                    let mut r = Reader::new(apdu);
                    let header = ConfirmedRequestHeader::decode(&mut r)?;
                    if header.service_choice != SERVICE_CONFIRMED_COV_NOTIFICATION {
                        continue;
                    }
                    if header.segmented {
                        return Err(ClientError::UnsupportedResponse);
                    }

                    let cov = CovNotificationRequest::decode_after_header(&mut r)?;
                    self.send_simple_ack(
                        source,
                        header.invoke_id,
                        SERVICE_CONFIRMED_COV_NOTIFICATION,
                    )
                    .await?;
                    return Ok(Some(into_client_cov_notification(source, true, cov)?));
                }
                _ => continue,
            }
        }

        Ok(None)
    }

    pub async fn recv_event_notification(
        &self,
        wait: Duration,
    ) -> Result<Option<EventNotification>, ClientError> {
        let _io_lock = self.request_io_lock.lock().await;
        let deadline = tokio::time::Instant::now() + wait;

        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let mut rx = [0u8; 1500];
            let recv = timeout(remaining, self.datalink.recv(&mut rx)).await;
            let (n, source) = match recv {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            };

            let apdu = extract_apdu(&rx[..n])?;
            let first = *apdu.first().ok_or(ClientError::UnsupportedResponse)?;
            match ApduType::from_u8(first >> 4) {
                Some(ApduType::UnconfirmedRequest) => {
                    let mut r = Reader::new(apdu);
                    let header = UnconfirmedRequestHeader::decode(&mut r)?;
                    if header.service_choice != SERVICE_UNCONFIRMED_EVENT_NOTIFICATION {
                        continue;
                    }
                    let notification = EventNotificationRequest::decode_after_header(&mut r)?;
                    return Ok(Some(into_client_event_notification(
                        source,
                        false,
                        notification,
                    )));
                }
                Some(ApduType::ConfirmedRequest) => {
                    let mut r = Reader::new(apdu);
                    let header = ConfirmedRequestHeader::decode(&mut r)?;
                    if header.service_choice != SERVICE_CONFIRMED_EVENT_NOTIFICATION {
                        continue;
                    }
                    if header.segmented {
                        return Err(ClientError::UnsupportedResponse);
                    }
                    let notification = EventNotificationRequest::decode_after_header(&mut r)?;
                    self.send_simple_ack(
                        source,
                        header.invoke_id,
                        SERVICE_CONFIRMED_EVENT_NOTIFICATION,
                    )
                    .await?;
                    return Ok(Some(into_client_event_notification(
                        source,
                        true,
                        notification,
                    )));
                }
                _ => continue,
            }
        }

        Ok(None)
    }

    pub async fn read_property(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
        property_id: PropertyId,
    ) -> Result<ClientDataValue, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let req = ReadPropertyRequest {
            object_id,
            property_id,
            array_index: None,
            invoke_id,
        };
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            req.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_READ_PROPERTY,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = ReadPropertyAck::decode_after_header(&mut pr)?;
        into_client_value(parsed.value)
    }

    pub async fn write_property(
        &self,
        address: DataLinkAddress,
        mut request: WritePropertyRequest<'_>,
    ) -> Result<(), ClientError> {
        request.invoke_id = self.next_invoke_id().await;
        let invoke_id = request.invoke_id;
        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            request.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_WRITE_PROPERTY,
            self.response_timeout,
        )
        .await
    }

    pub async fn read_property_multiple(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
        property_ids: &[PropertyId],
    ) -> Result<Vec<(PropertyId, ClientDataValue)>, ClientError> {
        let refs: Vec<PropertyReference> = property_ids
            .iter()
            .copied()
            .map(|property_id| PropertyReference {
                property_id,
                array_index: None,
            })
            .collect();
        let specs = [ReadAccessSpecification {
            object_id,
            properties: &refs,
        }];

        let invoke_id = self.next_invoke_id().await;
        let req = ReadPropertyMultipleRequest {
            specs: &specs,
            invoke_id,
        };

        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            req.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_READ_PROPERTY_MULTIPLE,
                self.response_timeout,
            )
            .await?;
        let mut pr = Reader::new(&payload);
        let parsed = ReadPropertyMultipleAck::decode_after_header(&mut pr)?;
        let mut out = Vec::new();
        for access in parsed.results {
            if access.object_id != object_id {
                continue;
            }
            for item in access.results {
                out.push((item.property_id, into_client_value(item.value)?));
            }
        }
        Ok(out)
    }

    pub async fn write_property_multiple(
        &self,
        address: DataLinkAddress,
        object_id: ObjectId,
        properties: &[PropertyWriteSpec<'_>],
    ) -> Result<(), ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let specs = [WriteAccessSpecification {
            object_id,
            properties,
        }];
        let req = WritePropertyMultipleRequest {
            specs: &specs,
            invoke_id,
        };

        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            req.encode(w)
        })?;
        self.await_simple_ack_or_error(
            address,
            &tx,
            invoke_id,
            SERVICE_WRITE_PROPERTY_MULTIPLE,
            self.response_timeout,
        )
        .await
    }

    /// Send a ConfirmedPrivateTransfer request and return the ack.
    pub async fn private_transfer(
        &self,
        address: DataLinkAddress,
        vendor_id: u32,
        service_number: u32,
        service_parameters: Option<&[u8]>,
    ) -> Result<PrivateTransferAck, ClientError> {
        let invoke_id = self.next_invoke_id().await;
        let req = ConfirmedPrivateTransferRequest {
            vendor_id,
            service_number,
            service_parameters,
            invoke_id,
        };

        let tx = self.encode_with_growth(|w| {
            Npdu::new(0).encode(w)?;
            req.encode(w)
        })?;
        let payload = self
            .await_complex_ack_payload_or_error(
                address,
                &tx,
                invoke_id,
                SERVICE_CONFIRMED_PRIVATE_TRANSFER,
                self.response_timeout,
            )
            .await?;
        let mut r = Reader::new(&payload);
        PrivateTransferAck::decode(&mut r).map_err(ClientError::from)
    }
}

fn extract_apdu(payload: &[u8]) -> Result<&[u8], ClientError> {
    let mut r = Reader::new(payload);
    let _npdu = Npdu::decode(&mut r)?;
    r.read_exact(r.remaining()).map_err(ClientError::from)
}

fn remote_service_error(err: BacnetError) -> ClientError {
    ClientError::RemoteServiceError {
        service_choice: err.service_choice,
        error_class_raw: err.error_class,
        error_code_raw: err.error_code,
        error_class: err.error_class.and_then(ErrorClass::from_u32),
        error_code: err.error_code.and_then(ErrorCode::from_u32),
    }
}

fn into_client_value(value: DataValue<'_>) -> Result<ClientDataValue, ClientError> {
    Ok(match value {
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
            let mut children = Vec::with_capacity(values.len());
            for child in values {
                children.push(into_client_value(child)?);
            }
            ClientDataValue::Constructed {
                tag_num,
                values: children,
            }
        }
    })
}

fn into_client_alarm_summary(value: Vec<CoreAlarmSummaryItem<'_>>) -> Vec<AlarmSummaryItem> {
    value
        .into_iter()
        .map(|item| AlarmSummaryItem {
            object_id: item.object_id,
            alarm_state_raw: item.alarm_state,
            alarm_state: rustbac_core::services::acknowledge_alarm::EventState::from_u32(
                item.alarm_state,
            ),
            acknowledged_transitions: ClientBitString {
                unused_bits: item.acknowledged_transitions.unused_bits,
                data: item.acknowledged_transitions.data.to_vec(),
            },
        })
        .collect()
}

fn into_client_enrollment_summary(
    value: Vec<CoreEnrollmentSummaryItem>,
) -> Vec<EnrollmentSummaryItem> {
    value
        .into_iter()
        .map(|item| EnrollmentSummaryItem {
            object_id: item.object_id,
            event_type: item.event_type,
            event_state_raw: item.event_state,
            event_state: rustbac_core::services::acknowledge_alarm::EventState::from_u32(
                item.event_state,
            ),
            priority: item.priority,
            notification_class: item.notification_class,
        })
        .collect()
}

fn into_client_event_information(
    value: Vec<CoreEventSummaryItem<'_>>,
) -> Vec<EventInformationItem> {
    value
        .into_iter()
        .map(|item| EventInformationItem {
            object_id: item.object_id,
            event_state_raw: item.event_state,
            event_state: rustbac_core::services::acknowledge_alarm::EventState::from_u32(
                item.event_state,
            ),
            acknowledged_transitions: ClientBitString {
                unused_bits: item.acknowledged_transitions.unused_bits,
                data: item.acknowledged_transitions.data.to_vec(),
            },
            notify_type: item.notify_type,
            event_enable: ClientBitString {
                unused_bits: item.event_enable.unused_bits,
                data: item.event_enable.data.to_vec(),
            },
            event_priorities: item.event_priorities,
        })
        .collect()
}

fn into_client_cov_notification(
    source: DataLinkAddress,
    confirmed: bool,
    value: CovNotificationRequest<'_>,
) -> Result<CovNotification, ClientError> {
    let mut values = Vec::with_capacity(value.values.len());
    for property in value.values {
        values.push(CovPropertyValue {
            property_id: property.property_id,
            array_index: property.array_index,
            value: into_client_value(property.value)?,
            priority: property.priority,
        });
    }

    Ok(CovNotification {
        source,
        confirmed,
        subscriber_process_id: value.subscriber_process_id,
        initiating_device_id: value.initiating_device_id,
        monitored_object_id: value.monitored_object_id,
        time_remaining_seconds: value.time_remaining_seconds,
        values,
    })
}

fn into_client_event_notification(
    source: DataLinkAddress,
    confirmed: bool,
    value: EventNotificationRequest<'_>,
) -> EventNotification {
    EventNotification {
        source,
        confirmed,
        process_id: value.process_id,
        initiating_device_id: value.initiating_device_id,
        event_object_id: value.event_object_id,
        timestamp: value.timestamp,
        notification_class: value.notification_class,
        priority: value.priority,
        event_type: value.event_type,
        message_text: value.message_text.map(str::to_string),
        notify_type: value.notify_type,
        ack_required: value.ack_required,
        from_state_raw: value.from_state,
        from_state: rustbac_core::services::acknowledge_alarm::EventState::from_u32(
            value.from_state,
        ),
        to_state_raw: value.to_state,
        to_state: rustbac_core::services::acknowledge_alarm::EventState::from_u32(value.to_state),
    }
}

fn into_client_read_range(value: ReadRangeAck<'_>) -> Result<ReadRangeResult, ClientError> {
    let mut items = Vec::with_capacity(value.items.len());
    for item in value.items {
        items.push(into_client_value(item)?);
    }
    Ok(ReadRangeResult {
        object_id: value.object_id,
        property_id: value.property_id,
        array_index: value.array_index,
        result_flags: ClientBitString {
            unused_bits: value.result_flags.unused_bits,
            data: value.result_flags.data.to_vec(),
        },
        item_count: value.item_count,
        items,
    })
}

fn into_client_atomic_read_result(value: AtomicReadFileAck<'_>) -> AtomicReadFileResult {
    match value.access_method {
        AtomicReadFileAckAccess::Stream {
            file_start_position,
            file_data,
        } => AtomicReadFileResult::Stream {
            end_of_file: value.end_of_file,
            file_start_position,
            file_data: file_data.to_vec(),
        },
        AtomicReadFileAckAccess::Record {
            file_start_record,
            returned_record_count,
            file_record_data,
        } => AtomicReadFileResult::Record {
            end_of_file: value.end_of_file,
            file_start_record,
            returned_record_count,
            file_record_data: file_record_data
                .into_iter()
                .map(|record| record.to_vec())
                .collect(),
        },
    }
}

fn into_client_atomic_write_result(value: AtomicWriteFileAck) -> AtomicWriteFileResult {
    match value {
        AtomicWriteFileAck::Stream {
            file_start_position,
        } => AtomicWriteFileResult::Stream {
            file_start_position,
        },
        AtomicWriteFileAck::Record { file_start_record } => {
            AtomicWriteFileResult::Record { file_start_record }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BacnetClient;
    use crate::{
        AlarmSummaryItem, AtomicReadFileResult, AtomicWriteFileResult, ClientDataValue,
        EnrollmentSummaryItem, EventInformationItem, EventNotification,
    };
    use rustbac_core::apdu::{
        ApduType, ComplexAckHeader, ConfirmedRequestHeader, SegmentAck, SimpleAck,
        UnconfirmedRequestHeader,
    };
    use rustbac_core::encoding::{
        primitives::{
            decode_signed, decode_unsigned, encode_app_real, encode_ctx_character_string,
            encode_ctx_object_id, encode_ctx_unsigned,
        },
        reader::Reader,
        tag::{AppTag, Tag},
        writer::Writer,
    };
    use rustbac_core::npdu::Npdu;
    use rustbac_core::services::acknowledge_alarm::{
        AcknowledgeAlarmRequest, EventState, TimeStamp, SERVICE_ACKNOWLEDGE_ALARM,
    };
    use rustbac_core::services::alarm_summary::SERVICE_GET_ALARM_SUMMARY;
    use rustbac_core::services::atomic_read_file::SERVICE_ATOMIC_READ_FILE;
    use rustbac_core::services::atomic_write_file::SERVICE_ATOMIC_WRITE_FILE;
    use rustbac_core::services::cov_notification::{
        SERVICE_CONFIRMED_COV_NOTIFICATION, SERVICE_UNCONFIRMED_COV_NOTIFICATION,
    };
    use rustbac_core::services::device_management::{
        DeviceCommunicationState, ReinitializeState, SERVICE_DEVICE_COMMUNICATION_CONTROL,
        SERVICE_REINITIALIZE_DEVICE,
    };
    use rustbac_core::services::enrollment_summary::SERVICE_GET_ENROLLMENT_SUMMARY;
    use rustbac_core::services::event_information::SERVICE_GET_EVENT_INFORMATION;
    use rustbac_core::services::event_notification::{
        SERVICE_CONFIRMED_EVENT_NOTIFICATION, SERVICE_UNCONFIRMED_EVENT_NOTIFICATION,
    };
    use rustbac_core::services::list_element::{
        AddListElementRequest, RemoveListElementRequest, SERVICE_ADD_LIST_ELEMENT,
        SERVICE_REMOVE_LIST_ELEMENT,
    };
    use rustbac_core::services::object_management::{SERVICE_CREATE_OBJECT, SERVICE_DELETE_OBJECT};
    use rustbac_core::services::read_property::SERVICE_READ_PROPERTY;
    use rustbac_core::services::read_property_multiple::SERVICE_READ_PROPERTY_MULTIPLE;
    use rustbac_core::services::read_range::SERVICE_READ_RANGE;
    use rustbac_core::services::subscribe_cov::{SubscribeCovRequest, SERVICE_SUBSCRIBE_COV};
    use rustbac_core::services::subscribe_cov_property::{
        SubscribeCovPropertyRequest, SERVICE_SUBSCRIBE_COV_PROPERTY,
    };
    use rustbac_core::services::time_synchronization::SERVICE_TIME_SYNCHRONIZATION;
    use rustbac_core::services::who_has::{SERVICE_I_HAVE, SERVICE_WHO_HAS};
    use rustbac_core::services::write_property_multiple::{
        PropertyWriteSpec, SERVICE_WRITE_PROPERTY_MULTIPLE,
    };
    use rustbac_core::types::{DataValue, Date, ObjectId, ObjectType, PropertyId, Time};
    use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    #[derive(Debug, Default)]
    struct MockState {
        sent: Mutex<Vec<(DataLinkAddress, Vec<u8>)>>,
        recv: Mutex<VecDeque<(Vec<u8>, DataLinkAddress)>>,
    }

    #[derive(Debug, Clone)]
    struct MockDataLink {
        state: Arc<MockState>,
    }

    impl MockDataLink {
        fn new() -> (Self, Arc<MockState>) {
            let state = Arc::new(MockState::default());
            (
                Self {
                    state: state.clone(),
                },
                state,
            )
        }
    }

    impl DataLink for MockDataLink {
        async fn send(
            &self,
            address: DataLinkAddress,
            payload: &[u8],
        ) -> Result<(), DataLinkError> {
            self.state
                .sent
                .lock()
                .await
                .push((address, payload.to_vec()));
            Ok(())
        }

        async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
            let Some((payload, addr)) = self.state.recv.lock().await.pop_front() else {
                return Err(DataLinkError::InvalidFrame);
            };
            if payload.len() > buf.len() {
                return Err(DataLinkError::FrameTooLarge);
            }
            buf[..payload.len()].copy_from_slice(&payload);
            Ok((payload.len(), addr))
        }
    }

    fn with_npdu(apdu: &[u8]) -> Vec<u8> {
        let mut out = [0u8; 512];
        let mut w = Writer::new(&mut out);
        Npdu::new(0).encode(&mut w).unwrap();
        w.write_all(apdu).unwrap();
        w.as_written().to_vec()
    }

    fn read_range_ack_apdu(invoke_id: u8, object_id: ObjectId) -> Vec<u8> {
        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_RANGE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, object_id.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Context { tag_num: 3, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();
        encode_ctx_unsigned(&mut w, 4, 2).unwrap();
        Tag::Opening { tag_num: 5 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 42.0).unwrap();
        encode_app_real(&mut w, 43.0).unwrap();
        Tag::Closing { tag_num: 5 }.encode(&mut w).unwrap();
        w.as_written().to_vec()
    }

    fn atomic_read_file_stream_ack_apdu(invoke_id: u8, eof: bool, data: &[u8]) -> Vec<u8> {
        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ATOMIC_READ_FILE,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Application {
            tag: AppTag::Boolean,
            len: if eof { 1 } else { 0 },
        }
        .encode(&mut w)
        .unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Application {
            tag: AppTag::SignedInt,
            len: 1,
        }
        .encode(&mut w)
        .unwrap();
        w.write_u8(0).unwrap();
        Tag::Application {
            tag: AppTag::OctetString,
            len: data.len() as u32,
        }
        .encode(&mut w)
        .unwrap();
        w.write_all(data).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        w.as_written().to_vec()
    }

    fn atomic_read_file_record_ack_apdu(invoke_id: u8) -> Vec<u8> {
        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ATOMIC_READ_FILE,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Application {
            tag: AppTag::Boolean,
            len: 0,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Opening { tag_num: 1 }.encode(&mut w).unwrap();
        Tag::Application {
            tag: AppTag::SignedInt,
            len: 1,
        }
        .encode(&mut w)
        .unwrap();
        w.write_u8(7).unwrap();
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len: 1,
        }
        .encode(&mut w)
        .unwrap();
        w.write_u8(2).unwrap();
        Tag::Application {
            tag: AppTag::OctetString,
            len: 2,
        }
        .encode(&mut w)
        .unwrap();
        w.write_all(&[0x01, 0x02]).unwrap();
        Tag::Application {
            tag: AppTag::OctetString,
            len: 3,
        }
        .encode(&mut w)
        .unwrap();
        w.write_all(&[0x03, 0x04, 0x05]).unwrap();
        Tag::Closing { tag_num: 1 }.encode(&mut w).unwrap();
        w.as_written().to_vec()
    }

    fn atomic_write_file_stream_ack_apdu(invoke_id: u8, start_position: i32) -> Vec<u8> {
        let mut apdu_buf = [0u8; 64];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ATOMIC_WRITE_FILE,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Context { tag_num: 0, len: 2 }.encode(&mut w).unwrap();
        w.write_all(&(start_position as i16).to_be_bytes()).unwrap();
        w.as_written().to_vec()
    }

    fn atomic_write_file_record_ack_apdu(invoke_id: u8, start_record: i32) -> Vec<u8> {
        let mut apdu_buf = [0u8; 64];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ATOMIC_WRITE_FILE,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Context { tag_num: 1, len: 1 }.encode(&mut w).unwrap();
        w.write_u8(start_record as u8).unwrap();
        w.as_written().to_vec()
    }

    fn create_object_ack_apdu(invoke_id: u8, object_id: ObjectId) -> Vec<u8> {
        let mut apdu_buf = [0u8; 64];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_CREATE_OBJECT,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, object_id.raw()).unwrap();
        w.as_written().to_vec()
    }

    fn get_alarm_summary_ack_apdu(invoke_id: u8) -> Vec<u8> {
        let mut apdu_buf = [0u8; 128];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_ALARM_SUMMARY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogInput, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 1).unwrap();
        Tag::Context { tag_num: 2, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();

        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::BinaryInput, 2).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 0).unwrap();
        Tag::Context { tag_num: 2, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1100_0000).unwrap();
        w.as_written().to_vec()
    }

    fn get_enrollment_summary_ack_apdu(invoke_id: u8) -> Vec<u8> {
        let mut apdu_buf = [0u8; 160];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_ENROLLMENT_SUMMARY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogInput, 7).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 1).unwrap();
        encode_ctx_unsigned(&mut w, 2, 2).unwrap();
        encode_ctx_unsigned(&mut w, 3, 200).unwrap();
        encode_ctx_unsigned(&mut w, 4, 10).unwrap();

        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::BinaryInput, 8).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 0).unwrap();
        encode_ctx_unsigned(&mut w, 2, 0).unwrap();
        encode_ctx_unsigned(&mut w, 3, 20).unwrap();
        encode_ctx_unsigned(&mut w, 4, 11).unwrap();
        w.as_written().to_vec()
    }

    fn get_event_information_ack_apdu(invoke_id: u8) -> Vec<u8> {
        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_GET_EVENT_INFORMATION,
        }
        .encode(&mut w)
        .unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::AnalogInput, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, 2).unwrap();
        Tag::Context { tag_num: 2, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 1, 1).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 4, 0).unwrap();
        Tag::Context { tag_num: 5, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1100_0000).unwrap();
        Tag::Opening { tag_num: 6 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        encode_ctx_unsigned(&mut w, 1, 2).unwrap();
        encode_ctx_unsigned(&mut w, 2, 3).unwrap();
        Tag::Closing { tag_num: 6 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Context { tag_num: 1, len: 1 }.encode(&mut w).unwrap();
        w.write_u8(0).unwrap();
        w.as_written().to_vec()
    }

    #[tokio::test]
    async fn who_has_object_name_collects_i_have() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 31], 47808).into());

        let mut apdu = [0u8; 128];
        let mut w = Writer::new(&mut apdu);
        UnconfirmedRequestHeader {
            service_choice: SERVICE_I_HAVE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::Device, 10).raw()).unwrap();
        encode_ctx_object_id(&mut w, 1, ObjectId::new(ObjectType::AnalogInput, 7).raw()).unwrap();
        encode_ctx_character_string(&mut w, 2, "Zone Temp").unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let results = client
            .who_has_object_name(None, "Zone Temp", Duration::from_millis(10))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].address, addr);
        assert_eq!(results[0].device_id, ObjectId::new(ObjectType::Device, 10));
        assert_eq!(
            results[0].object_id,
            ObjectId::new(ObjectType::AnalogInput, 7)
        );
        assert_eq!(results[0].object_name, "Zone Temp");

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_WHO_HAS);
    }

    #[tokio::test]
    async fn device_communication_control_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 32], 47808).into());

        let mut apdu = [0u8; 32];
        let mut w = Writer::new(&mut apdu);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_DEVICE_COMMUNICATION_CONTROL,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        client
            .device_communication_control(addr, Some(30), DeviceCommunicationState::Disable, None)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_DEVICE_COMMUNICATION_CONTROL);
    }

    #[tokio::test]
    async fn reinitialize_device_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 33], 47808).into());

        let mut apdu = [0u8; 32];
        let mut w = Writer::new(&mut apdu);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_REINITIALIZE_DEVICE,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        client
            .reinitialize_device(addr, ReinitializeState::ActivateChanges, Some("pw"))
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_REINITIALIZE_DEVICE);
    }

    #[tokio::test]
    async fn time_synchronize_sends_unconfirmed_request() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 34], 47808).into());

        client
            .time_synchronize(
                addr,
                Date {
                    year_since_1900: 126,
                    month: 2,
                    day: 7,
                    weekday: 6,
                },
                Time {
                    hour: 10,
                    minute: 11,
                    second: 12,
                    hundredths: 13,
                },
                false,
            )
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_TIME_SYNCHRONIZATION);
    }

    #[tokio::test]
    async fn get_alarm_summary_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 38], 47808).into());

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&get_alarm_summary_ack_apdu(1)), addr));

        let summaries = client.get_alarm_summary(addr).await.unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(
            summaries[0],
            AlarmSummaryItem {
                object_id: ObjectId::new(ObjectType::AnalogInput, 1),
                alarm_state_raw: 1,
                alarm_state: Some(EventState::Fault),
                acknowledged_transitions: crate::ClientBitString {
                    unused_bits: 5,
                    data: vec![0b1110_0000],
                },
            }
        );
        assert_eq!(
            summaries[1],
            AlarmSummaryItem {
                object_id: ObjectId::new(ObjectType::BinaryInput, 2),
                alarm_state_raw: 0,
                alarm_state: Some(EventState::Normal),
                acknowledged_transitions: crate::ClientBitString {
                    unused_bits: 5,
                    data: vec![0b1100_0000],
                },
            }
        );

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_GET_ALARM_SUMMARY);
    }

    #[tokio::test]
    async fn get_enrollment_summary_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 37], 47808).into());

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&get_enrollment_summary_ack_apdu(1)), addr));

        let summaries = client.get_enrollment_summary(addr).await.unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(
            summaries[0],
            EnrollmentSummaryItem {
                object_id: ObjectId::new(ObjectType::AnalogInput, 7),
                event_type: 1,
                event_state_raw: 2,
                event_state: Some(EventState::Offnormal),
                priority: 200,
                notification_class: 10,
            }
        );
        assert_eq!(
            summaries[1],
            EnrollmentSummaryItem {
                object_id: ObjectId::new(ObjectType::BinaryInput, 8),
                event_type: 0,
                event_state_raw: 0,
                event_state: Some(EventState::Normal),
                priority: 20,
                notification_class: 11,
            }
        );

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_GET_ENROLLMENT_SUMMARY);
    }

    #[tokio::test]
    async fn get_event_information_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 57], 47808).into());

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&get_event_information_ack_apdu(1)), addr));

        let result = client.get_event_information(addr, None).await.unwrap();
        assert!(!result.more_events);
        assert_eq!(result.summaries.len(), 1);
        assert_eq!(
            result.summaries[0],
            EventInformationItem {
                object_id: ObjectId::new(ObjectType::AnalogInput, 1),
                event_state_raw: 2,
                event_state: Some(EventState::Offnormal),
                acknowledged_transitions: crate::ClientBitString {
                    unused_bits: 5,
                    data: vec![0b1110_0000],
                },
                notify_type: 0,
                event_enable: crate::ClientBitString {
                    unused_bits: 5,
                    data: vec![0b1100_0000],
                },
                event_priorities: [1, 2, 3],
            }
        );
    }

    #[tokio::test]
    async fn acknowledge_alarm_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 39], 47808).into());

        let mut apdu = [0u8; 32];
        let mut w = Writer::new(&mut apdu);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_ACKNOWLEDGE_ALARM,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        client
            .acknowledge_alarm(
                addr,
                AcknowledgeAlarmRequest {
                    acknowledging_process_id: 10,
                    event_object_id: ObjectId::new(ObjectType::AnalogInput, 1),
                    event_state_acknowledged: EventState::Offnormal,
                    event_time_stamp: TimeStamp::SequenceNumber(42),
                    acknowledgment_source: "operator",
                    time_of_acknowledgment: TimeStamp::DateTime {
                        date: Date {
                            year_since_1900: 126,
                            month: 2,
                            day: 7,
                            weekday: 6,
                        },
                        time: Time {
                            hour: 10,
                            minute: 11,
                            second: 12,
                            hundredths: 13,
                        },
                    },
                    invoke_id: 0,
                },
            )
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_ACKNOWLEDGE_ALARM);
    }

    #[tokio::test]
    async fn create_object_by_type_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 50], 47808).into());
        let created = ObjectId::new(ObjectType::AnalogValue, 42);

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&create_object_ack_apdu(1, created)), addr));

        let result = client
            .create_object_by_type(addr, ObjectType::AnalogValue)
            .await
            .unwrap();
        assert_eq!(result, created);

        let sent = state.sent.lock().await;
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_CREATE_OBJECT);
    }

    #[tokio::test]
    async fn delete_object_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 51], 47808).into());

        let mut apdu = [0u8; 32];
        let mut w = Writer::new(&mut apdu);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_DELETE_OBJECT,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        client
            .delete_object(addr, ObjectId::new(ObjectType::AnalogValue, 42))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn add_list_element_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 52], 47808).into());

        let mut apdu = [0u8; 32];
        let mut w = Writer::new(&mut apdu);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_ADD_LIST_ELEMENT,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let values = [DataValue::Unsigned(1), DataValue::Unsigned(2)];
        client
            .add_list_element(
                addr,
                AddListElementRequest {
                    object_id: ObjectId::new(ObjectType::AnalogValue, 1),
                    property_id: PropertyId::Proprietary(512),
                    array_index: None,
                    elements: &values,
                    invoke_id: 0,
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remove_list_element_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 53], 47808).into());

        let mut apdu = [0u8; 32];
        let mut w = Writer::new(&mut apdu);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_REMOVE_LIST_ELEMENT,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let values = [DataValue::Unsigned(1)];
        client
            .remove_list_element(
                addr,
                RemoveListElementRequest {
                    object_id: ObjectId::new(ObjectType::AnalogValue, 1),
                    property_id: PropertyId::Proprietary(513),
                    array_index: None,
                    elements: &values,
                    invoke_id: 0,
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn atomic_read_file_stream_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 40], 47808).into());
        let file_object = ObjectId::new(ObjectType::File, 2);

        state.recv.lock().await.push_back((
            with_npdu(&atomic_read_file_stream_ack_apdu(
                1,
                true,
                &[0xAA, 0xBB, 0xCC],
            )),
            addr,
        ));

        let result = client
            .atomic_read_file_stream(addr, file_object, 0, 3)
            .await
            .unwrap();

        assert_eq!(
            result,
            AtomicReadFileResult::Stream {
                end_of_file: true,
                file_start_position: 0,
                file_data: vec![0xAA, 0xBB, 0xCC],
            }
        );

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_ATOMIC_READ_FILE);
    }

    #[tokio::test]
    async fn atomic_read_file_record_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 41], 47808).into());
        let file_object = ObjectId::new(ObjectType::File, 5);

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&atomic_read_file_record_ack_apdu(1)), addr));

        let result = client
            .atomic_read_file_record(addr, file_object, 7, 2)
            .await
            .unwrap();

        assert_eq!(
            result,
            AtomicReadFileResult::Record {
                end_of_file: false,
                file_start_record: 7,
                returned_record_count: 2,
                file_record_data: vec![vec![0x01, 0x02], vec![0x03, 0x04, 0x05]],
            }
        );
    }

    #[tokio::test]
    async fn atomic_write_file_stream_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 42], 47808).into());
        let file_object = ObjectId::new(ObjectType::File, 3);

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&atomic_write_file_stream_ack_apdu(1, 128)), addr));

        let result = client
            .atomic_write_file_stream(addr, file_object, 128, &[1, 2, 3, 4])
            .await
            .unwrap();

        assert_eq!(
            result,
            AtomicWriteFileResult::Stream {
                file_start_position: 128
            }
        );
    }

    #[tokio::test]
    async fn atomic_write_file_record_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 43], 47808).into());
        let file_object = ObjectId::new(ObjectType::File, 9);
        let records: [&[u8]; 2] = [&[0x10, 0x11], &[0x12]];

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&atomic_write_file_record_ack_apdu(1, 7)), addr));

        let result = client
            .atomic_write_file_record(addr, file_object, 7, &records)
            .await
            .unwrap();

        assert_eq!(
            result,
            AtomicWriteFileResult::Record {
                file_start_record: 7
            }
        );
    }

    #[tokio::test]
    async fn read_properties_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 5], 47808).into());
        let object_id = ObjectId::new(ObjectType::Device, 1);

        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 1,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, object_id.raw()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 1 }
            .encode(&mut w)
            .unwrap();
        encode_ctx_unsigned(&mut w, 2, PropertyId::PresentValue.to_u32()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 4 }
            .encode(&mut w)
            .unwrap();
        encode_app_real(&mut w, 55.5).unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 4 }
            .encode(&mut w)
            .unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 1 }
            .encode(&mut w)
            .unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let values = client
            .read_property_multiple(addr, object_id, &[PropertyId::PresentValue])
            .await
            .unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].0, PropertyId::PresentValue);
        assert!(matches!(values[0].1, ClientDataValue::Real(v) if (v - 55.5).abs() < f32::EPSILON));

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_READ_PROPERTY_MULTIPLE);
    }

    #[tokio::test]
    async fn read_property_multiple_reassembles_segmented_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 12], 47808).into());
        let object_id = ObjectId::new(ObjectType::Device, 1);

        let mut payload_buf = [0u8; 256];
        let mut pw = Writer::new(&mut payload_buf);
        encode_ctx_unsigned(&mut pw, 0, object_id.raw()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 1 }
            .encode(&mut pw)
            .unwrap();
        encode_ctx_unsigned(&mut pw, 2, PropertyId::PresentValue.to_u32()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 4 }
            .encode(&mut pw)
            .unwrap();
        encode_app_real(&mut pw, 66.0).unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 4 }
            .encode(&mut pw)
            .unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 1 }
            .encode(&mut pw)
            .unwrap();
        let payload = pw.as_written();
        let split = payload.len() / 2;

        let mut apdu1 = [0u8; 256];
        let mut w1 = Writer::new(&mut apdu1);
        ComplexAckHeader {
            segmented: true,
            more_follows: true,
            invoke_id: 1,
            sequence_number: Some(0),
            proposed_window_size: Some(1),
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w1)
        .unwrap();
        w1.write_all(&payload[..split]).unwrap();

        let mut apdu2 = [0u8; 256];
        let mut w2 = Writer::new(&mut apdu2);
        ComplexAckHeader {
            segmented: true,
            more_follows: false,
            invoke_id: 1,
            sequence_number: Some(1),
            proposed_window_size: Some(1),
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w2)
        .unwrap();
        w2.write_all(&payload[split..]).unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w1.as_written()), addr));
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w2.as_written()), addr));

        let values = client
            .read_property_multiple(addr, object_id, &[PropertyId::PresentValue])
            .await
            .unwrap();
        assert_eq!(values.len(), 1);
        assert!(matches!(values[0].1, ClientDataValue::Real(v) if (v - 66.0).abs() < f32::EPSILON));

        let sent = state.sent.lock().await;
        assert!(sent.len() >= 3);

        let mut saw_segment_ack = 0usize;
        for (_, frame) in sent.iter().skip(1) {
            let mut r = Reader::new(frame);
            let _npdu = Npdu::decode(&mut r).unwrap();
            let apdu = r.read_exact(r.remaining()).unwrap();
            if (apdu[0] >> 4) == ApduType::SegmentAck as u8 {
                let mut sr = Reader::new(apdu);
                let sack = SegmentAck::decode(&mut sr).unwrap();
                assert_eq!(sack.invoke_id, 1);
                saw_segment_ack += 1;
            }
        }
        assert!(saw_segment_ack >= 1);
    }

    #[tokio::test]
    async fn read_property_multiple_tolerates_duplicate_segment() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 18], 47808).into());
        let object_id = ObjectId::new(ObjectType::Device, 1);

        let mut payload_buf = [0u8; 256];
        let mut pw = Writer::new(&mut payload_buf);
        encode_ctx_unsigned(&mut pw, 0, object_id.raw()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 1 }
            .encode(&mut pw)
            .unwrap();
        encode_ctx_unsigned(&mut pw, 2, PropertyId::PresentValue.to_u32()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 4 }
            .encode(&mut pw)
            .unwrap();
        encode_app_real(&mut pw, 66.0).unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 4 }
            .encode(&mut pw)
            .unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 1 }
            .encode(&mut pw)
            .unwrap();
        let payload = pw.as_written();
        let split = payload.len() / 2;

        let mut apdu1 = [0u8; 256];
        let mut w1 = Writer::new(&mut apdu1);
        ComplexAckHeader {
            segmented: true,
            more_follows: true,
            invoke_id: 1,
            sequence_number: Some(0),
            proposed_window_size: Some(1),
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w1)
        .unwrap();
        w1.write_all(&payload[..split]).unwrap();

        let mut dup = [0u8; 256];
        let mut wd = Writer::new(&mut dup);
        ComplexAckHeader {
            segmented: true,
            more_follows: true,
            invoke_id: 1,
            sequence_number: Some(0),
            proposed_window_size: Some(1),
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut wd)
        .unwrap();
        wd.write_all(&payload[..split]).unwrap();

        let mut apdu2 = [0u8; 256];
        let mut w2 = Writer::new(&mut apdu2);
        ComplexAckHeader {
            segmented: true,
            more_follows: false,
            invoke_id: 1,
            sequence_number: Some(1),
            proposed_window_size: Some(1),
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w2)
        .unwrap();
        w2.write_all(&payload[split..]).unwrap();

        {
            let mut recv = state.recv.lock().await;
            recv.push_back((with_npdu(w1.as_written()), addr));
            recv.push_back((with_npdu(wd.as_written()), addr));
            recv.push_back((with_npdu(w2.as_written()), addr));
        }

        let values = client
            .read_property_multiple(addr, object_id, &[PropertyId::PresentValue])
            .await
            .unwrap();
        assert_eq!(values.len(), 1);
        assert!(matches!(values[0].1, ClientDataValue::Real(v) if (v - 66.0).abs() < f32::EPSILON));
    }

    #[tokio::test]
    async fn write_properties_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 6], 47808).into());
        let object_id = ObjectId::new(ObjectType::AnalogOutput, 2);

        let mut apdu_buf = [0u8; 32];
        let mut w = Writer::new(&mut apdu_buf);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let writes = [PropertyWriteSpec {
            property_id: PropertyId::PresentValue,
            array_index: None,
            value: DataValue::Real(12.5),
            priority: Some(8),
        }];
        client
            .write_property_multiple(addr, object_id, &writes)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_WRITE_PROPERTY_MULTIPLE);
    }

    #[tokio::test]
    async fn subscribe_cov_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 11], 47808).into());

        let mut apdu_buf = [0u8; 32];
        let mut w = Writer::new(&mut apdu_buf);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_SUBSCRIBE_COV,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        client
            .subscribe_cov(
                addr,
                SubscribeCovRequest {
                    subscriber_process_id: 10,
                    monitored_object_id: ObjectId::new(ObjectType::AnalogInput, 3),
                    issue_confirmed_notifications: Some(false),
                    lifetime_seconds: Some(300),
                    invoke_id: 0,
                },
            )
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_SUBSCRIBE_COV);
    }

    #[tokio::test]
    async fn subscribe_cov_property_handles_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 21], 47808).into());

        let mut apdu_buf = [0u8; 32];
        let mut w = Writer::new(&mut apdu_buf);
        SimpleAck {
            invoke_id: 1,
            service_choice: SERVICE_SUBSCRIBE_COV_PROPERTY,
        }
        .encode(&mut w)
        .unwrap();
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        client
            .subscribe_cov_property(
                addr,
                SubscribeCovPropertyRequest {
                    subscriber_process_id: 22,
                    monitored_object_id: ObjectId::new(ObjectType::AnalogInput, 3),
                    issue_confirmed_notifications: Some(true),
                    lifetime_seconds: Some(120),
                    monitored_property_id: PropertyId::PresentValue,
                    monitored_property_array_index: None,
                    cov_increment: Some(0.1),
                    invoke_id: 0,
                },
            )
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_SUBSCRIBE_COV_PROPERTY);
    }

    #[tokio::test]
    async fn read_range_by_position_decodes_complex_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 22], 47808).into());
        let object_id = ObjectId::new(ObjectType::TrendLog, 1);

        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 1,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_RANGE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_object_id(&mut w, 0, object_id.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Context { tag_num: 3, len: 2 }.encode(&mut w).unwrap();
        w.write_u8(5).unwrap();
        w.write_u8(0b1110_0000).unwrap();
        encode_ctx_unsigned(&mut w, 4, 2).unwrap();
        Tag::Opening { tag_num: 5 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 42.0).unwrap();
        encode_app_real(&mut w, 43.0).unwrap();
        Tag::Closing { tag_num: 5 }.encode(&mut w).unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let result = client
            .read_range_by_position(addr, object_id, PropertyId::PresentValue, None, 1, 2)
            .await
            .unwrap();
        assert_eq!(result.object_id, object_id);
        assert_eq!(result.item_count, 2);
        assert_eq!(result.items.len(), 2);
        assert!(matches!(
            result.items[0],
            ClientDataValue::Real(v) if (v - 42.0).abs() < f32::EPSILON
        ));
    }

    #[tokio::test]
    async fn read_range_by_sequence_number_encodes_range_selector() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 35], 47808).into());
        let object_id = ObjectId::new(ObjectType::TrendLog, 1);

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&read_range_ack_apdu(1, object_id)), addr));

        let _ = client
            .read_range_by_sequence_number(addr, object_id, PropertyId::PresentValue, None, 20, 2)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_READ_RANGE);
        match Tag::decode(&mut r).unwrap() {
            Tag::Context { tag_num: 0, len: 4 } => {
                let _ = r.read_exact(4).unwrap();
            }
            other => panic!("unexpected object id tag: {other:?}"),
        }
        match Tag::decode(&mut r).unwrap() {
            Tag::Context { tag_num: 1, len } => {
                let _ = decode_unsigned(&mut r, len as usize).unwrap();
            }
            other => panic!("unexpected property tag: {other:?}"),
        }
        assert_eq!(Tag::decode(&mut r).unwrap(), Tag::Opening { tag_num: 6 });
        match Tag::decode(&mut r).unwrap() {
            Tag::Application {
                tag: AppTag::UnsignedInt,
                len,
            } => {
                assert_eq!(decode_unsigned(&mut r, len as usize).unwrap(), 20);
            }
            other => panic!("unexpected ref seq tag: {other:?}"),
        }
        match Tag::decode(&mut r).unwrap() {
            Tag::Application {
                tag: AppTag::SignedInt,
                len,
            } => {
                assert_eq!(decode_signed(&mut r, len as usize).unwrap(), 2);
            }
            other => panic!("unexpected count tag: {other:?}"),
        }
        assert_eq!(Tag::decode(&mut r).unwrap(), Tag::Closing { tag_num: 6 });
    }

    #[tokio::test]
    async fn read_range_by_time_encodes_range_selector() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 36], 47808).into());
        let object_id = ObjectId::new(ObjectType::TrendLog, 1);
        let date = Date {
            year_since_1900: 126,
            month: 2,
            day: 7,
            weekday: 6,
        };
        let time = Time {
            hour: 10,
            minute: 11,
            second: 12,
            hundredths: 13,
        };

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(&read_range_ack_apdu(1, object_id)), addr));

        let _ = client
            .read_range_by_time(
                addr,
                object_id,
                PropertyId::PresentValue,
                None,
                (date, time),
                2,
            )
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_READ_RANGE);
        match Tag::decode(&mut r).unwrap() {
            Tag::Context { tag_num: 0, len: 4 } => {
                let _ = r.read_exact(4).unwrap();
            }
            other => panic!("unexpected object id tag: {other:?}"),
        }
        match Tag::decode(&mut r).unwrap() {
            Tag::Context { tag_num: 1, len } => {
                let _ = decode_unsigned(&mut r, len as usize).unwrap();
            }
            other => panic!("unexpected property tag: {other:?}"),
        }
        assert_eq!(Tag::decode(&mut r).unwrap(), Tag::Opening { tag_num: 7 });
        match Tag::decode(&mut r).unwrap() {
            Tag::Application {
                tag: AppTag::Date,
                len: 4,
            } => {
                let raw = r.read_exact(4).unwrap();
                assert_eq!(
                    raw,
                    &[date.year_since_1900, date.month, date.day, date.weekday]
                );
            }
            other => panic!("unexpected date tag: {other:?}"),
        }
        match Tag::decode(&mut r).unwrap() {
            Tag::Application {
                tag: AppTag::Time,
                len: 4,
            } => {
                let raw = r.read_exact(4).unwrap();
                assert_eq!(raw, &[time.hour, time.minute, time.second, time.hundredths]);
            }
            other => panic!("unexpected time tag: {other:?}"),
        }
        match Tag::decode(&mut r).unwrap() {
            Tag::Application {
                tag: AppTag::SignedInt,
                len,
            } => {
                assert_eq!(decode_signed(&mut r, len as usize).unwrap(), 2);
            }
            other => panic!("unexpected count tag: {other:?}"),
        }
        assert_eq!(Tag::decode(&mut r).unwrap(), Tag::Closing { tag_num: 7 });
    }

    #[tokio::test]
    async fn recv_unconfirmed_cov_notification_returns_decoded_value() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 12], 47808).into());

        let mut apdu = [0u8; 256];
        let mut w = Writer::new(&mut apdu);
        UnconfirmedRequestHeader {
            service_choice: SERVICE_UNCONFIRMED_COV_NOTIFICATION,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, 17).unwrap();
        encode_ctx_unsigned(&mut w, 1, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 2, ObjectId::new(ObjectType::AnalogInput, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 3, 60).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 73.25).unwrap();
        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let notification = client
            .recv_cov_notification(Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert!(!notification.confirmed);
        assert_eq!(notification.subscriber_process_id, 17);
        assert_eq!(notification.values.len(), 1);
        assert_eq!(notification.values[0].property_id, PropertyId::PresentValue);
        assert!(matches!(
            notification.values[0].value,
            ClientDataValue::Real(v) if (v - 73.25).abs() < f32::EPSILON
        ));

        let sent = state.sent.lock().await;
        assert!(sent.is_empty());
    }

    #[tokio::test]
    async fn recv_confirmed_cov_notification_sends_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 13], 47808).into());

        let mut apdu = [0u8; 256];
        let mut w = Writer::new(&mut apdu);
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: 9,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_CONFIRMED_COV_NOTIFICATION,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, 18).unwrap();
        encode_ctx_unsigned(&mut w, 1, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 2, ObjectId::new(ObjectType::AnalogInput, 2).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 3, 120).unwrap();
        Tag::Opening { tag_num: 4 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, PropertyId::PresentValue.to_u32()).unwrap();
        Tag::Opening { tag_num: 2 }.encode(&mut w).unwrap();
        encode_app_real(&mut w, 55.0).unwrap();
        Tag::Closing { tag_num: 2 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 4 }.encode(&mut w).unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let notification = client
            .recv_cov_notification(Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert!(notification.confirmed);
        assert_eq!(notification.values.len(), 1);

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let ack = SimpleAck::decode(&mut r).unwrap();
        assert_eq!(ack.invoke_id, 9);
        assert_eq!(ack.service_choice, SERVICE_CONFIRMED_COV_NOTIFICATION);
    }

    #[tokio::test]
    async fn recv_unconfirmed_event_notification_returns_decoded_value() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 16], 47808).into());

        let mut apdu = [0u8; 256];
        let mut w = Writer::new(&mut apdu);
        UnconfirmedRequestHeader {
            service_choice: SERVICE_UNCONFIRMED_EVENT_NOTIFICATION,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, 99).unwrap();
        encode_ctx_unsigned(&mut w, 1, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 2, ObjectId::new(ObjectType::AnalogInput, 6).raw()).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 1, 55).unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 4, 7).unwrap();
        encode_ctx_unsigned(&mut w, 5, 100).unwrap();
        encode_ctx_unsigned(&mut w, 6, 2).unwrap();
        encode_ctx_character_string(&mut w, 7, "fan alarm").unwrap();
        encode_ctx_unsigned(&mut w, 8, 0).unwrap();
        Tag::Context { tag_num: 9, len: 1 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 10, 2).unwrap();
        encode_ctx_unsigned(&mut w, 11, 0).unwrap();
        Tag::Opening { tag_num: 12 }.encode(&mut w).unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 12 }.encode(&mut w).unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let notification: EventNotification = client
            .recv_event_notification(Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert!(!notification.confirmed);
        assert_eq!(notification.process_id, 99);
        assert_eq!(notification.message_text.as_deref(), Some("fan alarm"));
        assert_eq!(notification.ack_required, Some(true));
        assert_eq!(notification.from_state, Some(EventState::Offnormal));
        assert_eq!(notification.to_state, Some(EventState::Normal));
        assert_eq!(notification.notify_type, 0);

        let sent = state.sent.lock().await;
        assert!(sent.is_empty());
    }

    #[tokio::test]
    async fn recv_confirmed_event_notification_sends_simple_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 17], 47808).into());

        let mut apdu = [0u8; 256];
        let mut w = Writer::new(&mut apdu);
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: 11,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_CONFIRMED_EVENT_NOTIFICATION,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, 100).unwrap();
        encode_ctx_unsigned(&mut w, 1, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
        encode_ctx_unsigned(&mut w, 2, ObjectId::new(ObjectType::AnalogInput, 7).raw()).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 1, 56).unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 4, 7).unwrap();
        encode_ctx_unsigned(&mut w, 5, 100).unwrap();
        encode_ctx_unsigned(&mut w, 6, 2).unwrap();
        encode_ctx_unsigned(&mut w, 8, 0).unwrap();
        encode_ctx_unsigned(&mut w, 10, 2).unwrap();
        encode_ctx_unsigned(&mut w, 11, 0).unwrap();
        Tag::Opening { tag_num: 12 }.encode(&mut w).unwrap();
        Tag::Opening { tag_num: 0 }.encode(&mut w).unwrap();
        encode_ctx_unsigned(&mut w, 0, 1).unwrap();
        Tag::Closing { tag_num: 0 }.encode(&mut w).unwrap();
        Tag::Closing { tag_num: 12 }.encode(&mut w).unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let notification = client
            .recv_event_notification(Duration::from_secs(1))
            .await
            .unwrap()
            .unwrap();
        assert!(notification.confirmed);

        let sent = state.sent.lock().await;
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let ack = SimpleAck::decode(&mut r).unwrap();
        assert_eq!(ack.invoke_id, 11);
        assert_eq!(ack.service_choice, SERVICE_CONFIRMED_EVENT_NOTIFICATION);
    }

    #[tokio::test]
    async fn write_property_multiple_segments_large_request() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 10], 47808).into());
        let object_id = ObjectId::new(ObjectType::AnalogOutput, 5);

        {
            let mut recv = state.recv.lock().await;
            for seq in 0u8..=254 {
                let mut apdu = [0u8; 16];
                let mut w = Writer::new(&mut apdu);
                SegmentAck {
                    negative_ack: false,
                    sent_by_server: true,
                    invoke_id: 1,
                    sequence_number: seq,
                    actual_window_size: 1,
                }
                .encode(&mut w)
                .unwrap();
                recv.push_back((with_npdu(w.as_written()), addr));
            }

            let mut apdu = [0u8; 16];
            let mut w = Writer::new(&mut apdu);
            SimpleAck {
                invoke_id: 1,
                service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
            }
            .encode(&mut w)
            .unwrap();
            recv.push_back((with_npdu(w.as_written()), addr));
        }

        let writes: Vec<PropertyWriteSpec> = (0..180)
            .map(|_| PropertyWriteSpec {
                property_id: PropertyId::Description,
                array_index: None,
                value: DataValue::CharacterString(
                    "rustbac segmented write test payload................................................................",
                ),
                priority: None,
            })
            .collect();

        client
            .write_property_multiple(addr, object_id, &writes)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert!(sent.len() > 1);

        let mut seqs = Vec::new();
        let mut saw_more_follows = false;
        let mut saw_last = false;
        for (_, frame) in sent.iter() {
            let mut r = Reader::new(frame);
            let _npdu = Npdu::decode(&mut r).unwrap();
            let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
            assert!(hdr.segmented);
            assert_eq!(hdr.service_choice, SERVICE_WRITE_PROPERTY_MULTIPLE);
            if hdr.more_follows {
                saw_more_follows = true;
            } else {
                saw_last = true;
            }
            seqs.push(hdr.sequence_number.unwrap());
        }

        assert!(saw_more_follows);
        assert!(saw_last);
        for (idx, seq) in seqs.iter().enumerate() {
            assert_eq!(*seq as usize, idx);
        }
    }

    #[tokio::test]
    async fn write_property_multiple_uses_configured_segment_window() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl)
            .with_response_timeout(Duration::from_secs(1))
            .with_segmented_request_window_size(4);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 14], 47808).into());
        let object_id = ObjectId::new(ObjectType::AnalogOutput, 6);

        {
            let mut recv = state.recv.lock().await;
            for seq in 0u8..=254 {
                let mut apdu = [0u8; 16];
                let mut w = Writer::new(&mut apdu);
                SegmentAck {
                    negative_ack: false,
                    sent_by_server: true,
                    invoke_id: 1,
                    sequence_number: seq,
                    actual_window_size: 4,
                }
                .encode(&mut w)
                .unwrap();
                recv.push_back((with_npdu(w.as_written()), addr));
            }

            let mut apdu = [0u8; 16];
            let mut w = Writer::new(&mut apdu);
            SimpleAck {
                invoke_id: 1,
                service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
            }
            .encode(&mut w)
            .unwrap();
            recv.push_back((with_npdu(w.as_written()), addr));
        }

        let writes: Vec<PropertyWriteSpec> = (0..180)
            .map(|_| PropertyWriteSpec {
                property_id: PropertyId::Description,
                array_index: None,
                value: DataValue::CharacterString(
                    "rustbac segmented write test payload................................................................",
                ),
                priority: None,
            })
            .collect();

        client
            .write_property_multiple(addr, object_id, &writes)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        assert!(sent.len() > 4);
        for (idx, (_, frame)) in sent.iter().take(4).enumerate() {
            let mut r = Reader::new(frame);
            let _npdu = Npdu::decode(&mut r).unwrap();
            let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
            assert!(hdr.segmented);
            assert_eq!(hdr.service_choice, SERVICE_WRITE_PROPERTY_MULTIPLE);
            assert_eq!(hdr.sequence_number, Some(idx as u8));
            assert_eq!(hdr.proposed_window_size, Some(4));
        }
    }

    #[tokio::test]
    async fn write_property_multiple_adapts_window_to_peer_ack_window() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl)
            .with_response_timeout(Duration::from_secs(1))
            .with_segmented_request_window_size(4);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 19], 47808).into());
        let object_id = ObjectId::new(ObjectType::AnalogOutput, 9);

        {
            let mut recv = state.recv.lock().await;
            for seq in 0u8..=254 {
                let mut apdu = [0u8; 16];
                let mut w = Writer::new(&mut apdu);
                SegmentAck {
                    negative_ack: false,
                    sent_by_server: true,
                    invoke_id: 1,
                    sequence_number: seq,
                    actual_window_size: 2,
                }
                .encode(&mut w)
                .unwrap();
                recv.push_back((with_npdu(w.as_written()), addr));
            }

            let mut apdu = [0u8; 16];
            let mut w = Writer::new(&mut apdu);
            SimpleAck {
                invoke_id: 1,
                service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
            }
            .encode(&mut w)
            .unwrap();
            recv.push_back((with_npdu(w.as_written()), addr));
        }

        let writes: Vec<PropertyWriteSpec> = (0..180)
            .map(|_| PropertyWriteSpec {
                property_id: PropertyId::Description,
                array_index: None,
                value: DataValue::CharacterString(
                    "rustbac segmented write test payload................................................................",
                ),
                priority: None,
            })
            .collect();

        client
            .write_property_multiple(addr, object_id, &writes)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        let mut saw_adapted_window = false;
        for (_, frame) in sent.iter() {
            let mut r = Reader::new(frame);
            let _npdu = Npdu::decode(&mut r).unwrap();
            let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
            if hdr.sequence_number.unwrap_or(0) >= 4 && hdr.proposed_window_size == Some(2) {
                saw_adapted_window = true;
                break;
            }
        }
        assert!(saw_adapted_window);
    }

    #[tokio::test]
    async fn write_property_multiple_retries_segment_batch_on_negative_ack() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl)
            .with_response_timeout(Duration::from_secs(1))
            .with_segmented_request_window_size(1)
            .with_segmented_request_retries(1);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 15], 47808).into());
        let object_id = ObjectId::new(ObjectType::AnalogOutput, 7);

        {
            let mut recv = state.recv.lock().await;

            let mut nack_apdu = [0u8; 16];
            let mut nack_w = Writer::new(&mut nack_apdu);
            SegmentAck {
                negative_ack: true,
                sent_by_server: true,
                invoke_id: 1,
                sequence_number: 0,
                actual_window_size: 1,
            }
            .encode(&mut nack_w)
            .unwrap();
            recv.push_back((with_npdu(nack_w.as_written()), addr));

            for seq in 0u8..=254 {
                let mut apdu = [0u8; 16];
                let mut w = Writer::new(&mut apdu);
                SegmentAck {
                    negative_ack: false,
                    sent_by_server: true,
                    invoke_id: 1,
                    sequence_number: seq,
                    actual_window_size: 1,
                }
                .encode(&mut w)
                .unwrap();
                recv.push_back((with_npdu(w.as_written()), addr));
            }

            let mut apdu = [0u8; 16];
            let mut w = Writer::new(&mut apdu);
            SimpleAck {
                invoke_id: 1,
                service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
            }
            .encode(&mut w)
            .unwrap();
            recv.push_back((with_npdu(w.as_written()), addr));
        }

        let writes: Vec<PropertyWriteSpec> = (0..180)
            .map(|_| PropertyWriteSpec {
                property_id: PropertyId::Description,
                array_index: None,
                value: DataValue::CharacterString(
                    "rustbac segmented write test payload................................................................",
                ),
                priority: None,
            })
            .collect();

        client
            .write_property_multiple(addr, object_id, &writes)
            .await
            .unwrap();

        let sent = state.sent.lock().await;
        let mut seq0_frames = 0usize;
        for (_, frame) in sent.iter() {
            let mut r = Reader::new(frame);
            let _npdu = Npdu::decode(&mut r).unwrap();
            let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
            if hdr.sequence_number == Some(0) {
                seq0_frames += 1;
            }
        }
        assert!(seq0_frames >= 2);
    }

    #[tokio::test]
    async fn read_property_ignores_invalid_frames_until_valid_response() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl).with_response_timeout(Duration::from_secs(1));
        let addr = DataLinkAddress::Ip(([192, 168, 1, 16], 47808).into());
        let state_for_task = state.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let mut apdu = [0u8; 128];
            let mut w = Writer::new(&mut apdu);
            ComplexAckHeader {
                segmented: false,
                more_follows: false,
                invoke_id: 1,
                sequence_number: None,
                proposed_window_size: None,
                service_choice: SERVICE_READ_PROPERTY,
            }
            .encode(&mut w)
            .unwrap();
            encode_ctx_object_id(&mut w, 0, ObjectId::new(ObjectType::Device, 1).raw()).unwrap();
            encode_ctx_unsigned(&mut w, 1, PropertyId::PresentValue.to_u32()).unwrap();
            Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
            encode_app_real(&mut w, 77.0).unwrap();
            Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();
            state_for_task
                .recv
                .lock()
                .await
                .push_back((with_npdu(w.as_written()), addr));
        });

        let value = client
            .read_property(
                addr,
                ObjectId::new(ObjectType::Device, 1),
                PropertyId::PresentValue,
            )
            .await
            .unwrap();
        assert!(matches!(
            value,
            ClientDataValue::Real(v) if (v - 77.0).abs() < f32::EPSILON
        ));
    }

    #[tokio::test]
    async fn read_property_maps_reject() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 7], 47808).into());

        let mut apdu = [0u8; 8];
        let mut w = Writer::new(&mut apdu);
        w.write_u8((ApduType::Reject as u8) << 4).unwrap();
        w.write_u8(1).unwrap(); // invoke id
        w.write_u8(2).unwrap(); // reason
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let err = client
            .read_property(
                addr,
                ObjectId::new(ObjectType::Device, 1),
                PropertyId::ObjectName,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            crate::ClientError::RemoteReject { reason: 2 }
        ));
    }

    #[tokio::test]
    async fn read_property_maps_remote_error_details() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 17], 47808).into());

        let mut apdu = [0u8; 16];
        let mut w = Writer::new(&mut apdu);
        w.write_u8((ApduType::Error as u8) << 4).unwrap();
        w.write_u8(1).unwrap(); // invoke id
        w.write_u8(rustbac_core::services::read_property::SERVICE_READ_PROPERTY)
            .unwrap();
        Tag::Context { tag_num: 0, len: 1 }.encode(&mut w).unwrap();
        w.write_u8(2).unwrap(); // property class
        Tag::Context { tag_num: 1, len: 1 }.encode(&mut w).unwrap();
        w.write_u8(32).unwrap(); // unknownProperty

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let err = client
            .read_property(
                addr,
                ObjectId::new(ObjectType::Device, 1),
                PropertyId::ObjectName,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            crate::ClientError::RemoteServiceError {
                service_choice: rustbac_core::services::read_property::SERVICE_READ_PROPERTY,
                error_class_raw: Some(2),
                error_code_raw: Some(32),
                error_class: Some(rustbac_core::types::ErrorClass::Property),
                error_code: Some(rustbac_core::types::ErrorCode::UnknownProperty),
            }
        ));
    }

    #[tokio::test]
    async fn write_property_maps_abort() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 8], 47808).into());

        let mut apdu = [0u8; 8];
        let mut w = Writer::new(&mut apdu);
        w.write_u8(((ApduType::Abort as u8) << 4) | 0x01).unwrap(); // server abort
        w.write_u8(1).unwrap(); // invoke id
        w.write_u8(9).unwrap(); // reason
        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let req = rustbac_core::services::write_property::WritePropertyRequest {
            object_id: ObjectId::new(ObjectType::AnalogOutput, 1),
            property_id: PropertyId::PresentValue,
            value: DataValue::Real(10.0),
            priority: Some(8),
            ..Default::default()
        };
        let err = client.write_property(addr, req).await.unwrap_err();
        assert!(matches!(
            err,
            crate::ClientError::RemoteAbort {
                reason: 9,
                server: true
            }
        ));
    }

    #[tokio::test]
    async fn read_property_multiple_returns_owned_string() {
        let (dl, state) = MockDataLink::new();
        let client = BacnetClient::with_datalink(dl);
        let addr = DataLinkAddress::Ip(([192, 168, 1, 9], 47808).into());
        let object_id = ObjectId::new(ObjectType::Device, 1);

        let mut apdu_buf = [0u8; 256];
        let mut w = Writer::new(&mut apdu_buf);
        ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id: 1,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, object_id.raw()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 1 }
            .encode(&mut w)
            .unwrap();
        encode_ctx_unsigned(&mut w, 2, PropertyId::ObjectName.to_u32()).unwrap();
        rustbac_core::encoding::tag::Tag::Opening { tag_num: 4 }
            .encode(&mut w)
            .unwrap();
        rustbac_core::services::value_codec::encode_application_data_value(
            &mut w,
            &DataValue::CharacterString("AHU-1"),
        )
        .unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 4 }
            .encode(&mut w)
            .unwrap();
        rustbac_core::encoding::tag::Tag::Closing { tag_num: 1 }
            .encode(&mut w)
            .unwrap();

        state
            .recv
            .lock()
            .await
            .push_back((with_npdu(w.as_written()), addr));

        let values = client
            .read_property_multiple(addr, object_id, &[PropertyId::ObjectName])
            .await
            .unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].0, PropertyId::ObjectName);
        assert!(matches!(
            &values[0].1,
            ClientDataValue::CharacterString(s) if s == "AHU-1"
        ));
    }

    #[tokio::test]
    async fn new_sc_rejects_invalid_endpoint() {
        let err = BacnetClient::new_sc("not a url").await.unwrap_err();
        assert!(matches!(err, crate::ClientError::DataLink(_)));
    }
}
