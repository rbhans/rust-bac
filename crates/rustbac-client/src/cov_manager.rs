use crate::{BacnetClient, CovNotification, CovPropertyValue};
use rustbac_core::services::subscribe_cov::SubscribeCovRequest;
use rustbac_core::services::subscribe_cov_property::SubscribeCovPropertyRequest;
use rustbac_core::types::{ObjectId, PropertyId};
use rustbac_datalink::{DataLink, DataLinkAddress};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

/// Source of a [`CovUpdate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UpdateSource {
    Cov,
    Poll,
}

/// A managed COV subscription spec.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CovSubscriptionSpec {
    pub address: DataLinkAddress,
    pub object_id: ObjectId,
    pub property_id: Option<PropertyId>,
    pub lifetime_seconds: u32,
    pub cov_increment: Option<f32>,
    pub confirmed: bool,
    pub subscriber_process_id: u32,
}

/// A single update emitted by [`CovManager`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CovUpdate {
    pub address: DataLinkAddress,
    pub object_id: ObjectId,
    pub values: Vec<CovPropertyValue>,
    pub source: UpdateSource,
}

/// Background COV manager handle.
#[derive(Debug)]
pub struct CovManager {
    thread: Option<std::thread::JoinHandle<()>>,
    shutdown: watch::Sender<bool>,
    rx: mpsc::UnboundedReceiver<CovUpdate>,
}

impl CovManager {
    /// Receive the next update from the manager.
    pub async fn recv(&mut self) -> Option<CovUpdate> {
        self.rx.recv().await
    }

    /// Stop the manager task.
    pub fn stop(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for CovManager {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// Builder for [`CovManager`].
pub struct CovManagerBuilder<D: DataLink> {
    client: Arc<BacnetClient<D>>,
    subscriptions: Vec<CovSubscriptionSpec>,
    poll_interval: Duration,
    silence_threshold: Duration,
    renewal_fraction: f64,
}

impl<D: DataLink + 'static> CovManagerBuilder<D> {
    pub fn new(client: Arc<BacnetClient<D>>) -> Self {
        Self {
            client,
            subscriptions: Vec::new(),
            poll_interval: Duration::from_secs(30),
            silence_threshold: Duration::from_secs(5 * 60),
            renewal_fraction: 0.75,
        }
    }

    pub fn subscribe(mut self, spec: CovSubscriptionSpec) -> Self {
        self.subscriptions.push(spec);
        self
    }

    pub fn poll_interval(mut self, duration: Duration) -> Self {
        self.poll_interval = duration;
        self
    }

    pub fn silence_threshold(mut self, duration: Duration) -> Self {
        self.silence_threshold = duration;
        self
    }

    pub fn renewal_fraction(mut self, fraction: f64) -> Self {
        self.renewal_fraction = fraction;
        self
    }

    pub fn build(self) -> CovManager {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let poll_interval = self.poll_interval.max(Duration::from_millis(1));
        let silence_threshold = self.silence_threshold.max(Duration::from_millis(1));
        let renewal_fraction = sanitize_fraction(self.renewal_fraction);
        let client = self.client;
        let subscriptions = self.subscriptions;
        let runtime_handle = tokio::runtime::Handle::current();

        let thread = std::thread::spawn(move || {
            runtime_handle.block_on(async move {
                run_cov_manager(
                    client,
                    subscriptions,
                    tx,
                    shutdown_rx,
                    poll_interval,
                    silence_threshold,
                    renewal_fraction,
                )
                .await;
            });
        });
        CovManager {
            thread: Some(thread),
            shutdown: shutdown_tx,
            rx,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriptionMode {
    Cov,
    Polling,
}

#[derive(Debug, Clone)]
struct SubscriptionState {
    spec: CovSubscriptionSpec,
    mode: SubscriptionMode,
    last_notification: Option<Instant>,
    next_renewal: Instant,
    next_poll: Instant,
}

impl SubscriptionState {
    fn new(
        spec: CovSubscriptionSpec,
        poll_interval: Duration,
        renewal_fraction: f64,
        now: Instant,
    ) -> Self {
        let lifetime_seconds = spec.lifetime_seconds;
        Self {
            spec,
            mode: SubscriptionMode::Cov,
            last_notification: None,
            next_renewal: now + renewal_delay_seconds(lifetime_seconds, renewal_fraction),
            next_poll: now + poll_interval,
        }
    }

    fn on_subscribe_success(
        &mut self,
        now: Instant,
        renewal_fraction: f64,
        poll_interval: Duration,
    ) {
        self.mode = SubscriptionMode::Cov;
        self.last_notification = Some(now);
        self.next_renewal =
            now + renewal_delay_seconds(self.spec.lifetime_seconds, renewal_fraction);
        self.next_poll = now + poll_interval;
    }

    fn on_subscribe_failure(&mut self, now: Instant, renewal_fraction: f64) {
        self.mode = SubscriptionMode::Polling;
        self.next_renewal =
            now + renewal_delay_seconds(self.spec.lifetime_seconds, renewal_fraction);
        self.next_poll = now;
    }

    fn is_silent(&self, now: Instant, threshold: Duration) -> bool {
        self.mode == SubscriptionMode::Cov
            && self
                .last_notification
                .map(|last| now.saturating_duration_since(last) > threshold)
                .unwrap_or(false)
    }
}

async fn run_cov_manager<D: DataLink>(
    client: Arc<BacnetClient<D>>,
    subscriptions: Vec<CovSubscriptionSpec>,
    tx: mpsc::UnboundedSender<CovUpdate>,
    mut shutdown_rx: watch::Receiver<bool>,
    poll_interval: Duration,
    silence_threshold: Duration,
    renewal_fraction: f64,
) {
    if subscriptions.is_empty() {
        return;
    }

    let now = Instant::now();
    let mut states: Vec<SubscriptionState> = subscriptions
        .into_iter()
        .map(|spec| SubscriptionState::new(spec, poll_interval, renewal_fraction, now))
        .collect();

    for state in &mut states {
        let attempt = subscribe_spec(&client, &state.spec).await;
        let now = Instant::now();
        if attempt {
            state.on_subscribe_success(now, renewal_fraction, poll_interval);
        } else {
            state.on_subscribe_failure(now, renewal_fraction);
        }
    }

    let listen_window = poll_interval
        .min(Duration::from_secs(1))
        .max(Duration::from_millis(50));

    loop {
        if *shutdown_rx.borrow() {
            return;
        }

        let recv_result = tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    return;
                }
                continue;
            }
            recv_result = client.recv_cov_notification(listen_window) => recv_result,
        };

        match recv_result {
            Ok(Some(notification)) => {
                let now = Instant::now();
                for state in &mut states {
                    if !notification_matches_spec(&notification, &state.spec) {
                        continue;
                    }

                    state.last_notification = Some(now);
                    state.mode = SubscriptionMode::Cov;

                    let values = filter_cov_values(&notification.values, state.spec.property_id);
                    if state.spec.property_id.is_some() && values.is_empty() {
                        continue;
                    }

                    let update = CovUpdate {
                        address: state.spec.address,
                        object_id: state.spec.object_id,
                        values,
                        source: UpdateSource::Cov,
                    };
                    if tx.send(update).is_err() {
                        return;
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                log::debug!("cov manager recv error: {err}");
            }
        }

        let now = Instant::now();
        for state in &mut states {
            if now >= state.next_renewal {
                if subscribe_spec(&client, &state.spec).await {
                    state.on_subscribe_success(Instant::now(), renewal_fraction, poll_interval);
                } else {
                    state.on_subscribe_failure(Instant::now(), renewal_fraction);
                }
            }

            if state.is_silent(now, silence_threshold) {
                state.mode = SubscriptionMode::Polling;
                if subscribe_spec(&client, &state.spec).await {
                    state.on_subscribe_success(Instant::now(), renewal_fraction, poll_interval);
                } else {
                    state.on_subscribe_failure(Instant::now(), renewal_fraction);
                }
            }

            if state.mode == SubscriptionMode::Polling && now >= state.next_poll {
                if subscribe_spec(&client, &state.spec).await {
                    state.on_subscribe_success(Instant::now(), renewal_fraction, poll_interval);
                    continue;
                }

                if let Some(update) = poll_spec(&client, &state.spec).await {
                    if tx.send(update).is_err() {
                        return;
                    }
                }
                state.next_poll = Instant::now() + poll_interval;
            }
        }
    }
}

async fn subscribe_spec<D: DataLink>(client: &BacnetClient<D>, spec: &CovSubscriptionSpec) -> bool {
    match spec.property_id {
        Some(property_id) => client
            .subscribe_cov_property(
                spec.address,
                SubscribeCovPropertyRequest {
                    subscriber_process_id: spec.subscriber_process_id,
                    monitored_object_id: spec.object_id,
                    issue_confirmed_notifications: Some(spec.confirmed),
                    lifetime_seconds: Some(spec.lifetime_seconds),
                    monitored_property_id: property_id,
                    monitored_property_array_index: None,
                    cov_increment: spec.cov_increment,
                    invoke_id: 0,
                },
            )
            .await
            .is_ok(),
        None => client
            .subscribe_cov(
                spec.address,
                SubscribeCovRequest {
                    subscriber_process_id: spec.subscriber_process_id,
                    monitored_object_id: spec.object_id,
                    issue_confirmed_notifications: Some(spec.confirmed),
                    lifetime_seconds: Some(spec.lifetime_seconds),
                    invoke_id: 0,
                },
            )
            .await
            .is_ok(),
    }
}

async fn poll_spec<D: DataLink>(
    client: &BacnetClient<D>,
    spec: &CovSubscriptionSpec,
) -> Option<CovUpdate> {
    let property_id = spec.property_id.unwrap_or(PropertyId::PresentValue);
    let value = client
        .read_property(spec.address, spec.object_id, property_id)
        .await
        .ok()?;

    Some(CovUpdate {
        address: spec.address,
        object_id: spec.object_id,
        values: vec![CovPropertyValue {
            property_id,
            array_index: None,
            value,
            priority: None,
        }],
        source: UpdateSource::Poll,
    })
}

fn notification_matches_spec(notification: &CovNotification, spec: &CovSubscriptionSpec) -> bool {
    notification.source == spec.address
        && notification.monitored_object_id == spec.object_id
        && notification.subscriber_process_id == spec.subscriber_process_id
}

fn filter_cov_values(
    values: &[CovPropertyValue],
    property_id: Option<PropertyId>,
) -> Vec<CovPropertyValue> {
    match property_id {
        Some(property_id) => values
            .iter()
            .filter(|value| value.property_id == property_id)
            .cloned()
            .collect(),
        None => values.to_vec(),
    }
}

fn sanitize_fraction(fraction: f64) -> f64 {
    if !fraction.is_finite() {
        return 0.75;
    }
    fraction.clamp(0.01, 1.0)
}

fn renewal_delay_seconds(lifetime_seconds: u32, renewal_fraction: f64) -> Duration {
    let seconds = ((lifetime_seconds.max(1) as f64) * sanitize_fraction(renewal_fraction))
        .round()
        .max(1.0);
    Duration::from_secs(seconds as u64)
}

#[cfg(test)]
mod tests {
    use super::{
        notification_matches_spec, renewal_delay_seconds, CovManagerBuilder, CovSubscriptionSpec,
        SubscriptionMode, SubscriptionState, UpdateSource,
    };
    use crate::{BacnetClient, ClientDataValue, CovNotification, SimulatedDevice};
    use rustbac_core::types::{ObjectId, ObjectType, PropertyId};
    use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{mpsc, Mutex};
    use tokio::time::{timeout, Instant};

    #[derive(Clone)]
    struct ChannelDataLink {
        local_addr: DataLinkAddress,
        tx: mpsc::UnboundedSender<(Vec<u8>, DataLinkAddress)>,
        rx: Arc<Mutex<mpsc::UnboundedReceiver<(Vec<u8>, DataLinkAddress)>>>,
    }

    impl DataLink for ChannelDataLink {
        async fn send(
            &self,
            _address: DataLinkAddress,
            payload: &[u8],
        ) -> Result<(), DataLinkError> {
            self.tx
                .send((payload.to_vec(), self.local_addr))
                .map_err(|_| DataLinkError::InvalidFrame)
        }

        async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
            let mut rx = self.rx.lock().await;
            let Some((payload, source)) = rx.recv().await else {
                return Err(DataLinkError::InvalidFrame);
            };
            if payload.len() > buf.len() {
                return Err(DataLinkError::FrameTooLarge);
            }
            buf[..payload.len()].copy_from_slice(&payload);
            Ok((payload.len(), source))
        }
    }

    fn datalink_pair() -> (ChannelDataLink, ChannelDataLink, DataLinkAddress) {
        let client_addr =
            DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47820));
        let simulator_addr =
            DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47821));
        let (client_tx, simulator_rx) = mpsc::unbounded_channel();
        let (simulator_tx, client_rx) = mpsc::unbounded_channel();

        (
            ChannelDataLink {
                local_addr: client_addr,
                tx: client_tx,
                rx: Arc::new(Mutex::new(client_rx)),
            },
            ChannelDataLink {
                local_addr: simulator_addr,
                tx: simulator_tx,
                rx: Arc::new(Mutex::new(simulator_rx)),
            },
            simulator_addr,
        )
    }

    #[test]
    fn state_transitions_cov_to_polling_to_cov() {
        let now = Instant::now();
        let spec = CovSubscriptionSpec {
            address: DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47830)),
            object_id: ObjectId::new(ObjectType::AnalogInput, 1),
            property_id: Some(PropertyId::PresentValue),
            lifetime_seconds: 60,
            cov_increment: None,
            confirmed: false,
            subscriber_process_id: 1,
        };
        let mut state = SubscriptionState::new(spec, Duration::from_secs(1), 0.75, now);
        state.mode = SubscriptionMode::Cov;
        state.last_notification = Some(now - Duration::from_secs(10));

        assert!(state.is_silent(now, Duration::from_secs(5)));

        state.on_subscribe_failure(now, 0.75);
        assert_eq!(state.mode, SubscriptionMode::Polling);

        state.on_subscribe_success(now, 0.75, Duration::from_secs(1));
        assert_eq!(state.mode, SubscriptionMode::Cov);
        assert_eq!(state.last_notification, Some(now));
    }

    #[test]
    fn renewal_fraction_scales_lifetime() {
        assert_eq!(renewal_delay_seconds(120, 0.75), Duration::from_secs(90));
        assert_eq!(renewal_delay_seconds(120, 1.5), Duration::from_secs(120));
        assert_eq!(renewal_delay_seconds(120, 0.0), Duration::from_secs(1));
    }

    #[test]
    fn matching_includes_subscriber_process_id() {
        let address = DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47830));
        let object_id = ObjectId::new(ObjectType::AnalogInput, 1);
        let spec = CovSubscriptionSpec {
            address,
            object_id,
            property_id: None,
            lifetime_seconds: 60,
            cov_increment: None,
            confirmed: false,
            subscriber_process_id: 42,
        };
        let mut notification = CovNotification {
            source: address,
            confirmed: false,
            subscriber_process_id: 7,
            initiating_device_id: ObjectId::new(ObjectType::Device, 100),
            monitored_object_id: object_id,
            time_remaining_seconds: 30,
            values: vec![],
        };
        assert!(!notification_matches_spec(&notification, &spec));

        notification.subscriber_process_id = 42;
        assert!(notification_matches_spec(&notification, &spec));
    }

    #[tokio::test]
    async fn polling_fallback_emits_updates_with_simulator() {
        let (client_dl, simulator_dl, simulator_addr) = datalink_pair();

        let simulator = SimulatedDevice::new(2000, simulator_dl);
        let object_id = ObjectId::new(ObjectType::AnalogInput, 1);
        let mut props = HashMap::new();
        props.insert(PropertyId::PresentValue, ClientDataValue::Real(42.0));
        props.insert(
            PropertyId::ObjectName,
            ClientDataValue::CharacterString("AI-1".to_string()),
        );
        simulator.add_object(object_id, props).await;

        let simulator_task = tokio::spawn(async move {
            let _ = simulator.run().await;
        });

        let client = Arc::new(
            BacnetClient::with_datalink(client_dl).with_response_timeout(Duration::from_millis(50)),
        );

        let spec = CovSubscriptionSpec {
            address: simulator_addr,
            object_id,
            property_id: Some(PropertyId::PresentValue),
            lifetime_seconds: 30,
            cov_increment: None,
            confirmed: false,
            subscriber_process_id: 99,
        };

        let mut manager = CovManagerBuilder::new(client)
            .subscribe(spec)
            .poll_interval(Duration::from_millis(75))
            .silence_threshold(Duration::from_millis(200))
            .build();

        let update = timeout(Duration::from_secs(2), manager.recv())
            .await
            .expect("manager recv timed out")
            .expect("manager channel closed unexpectedly");

        assert_eq!(update.source, UpdateSource::Poll);
        assert_eq!(update.object_id, object_id);
        assert_eq!(update.values.len(), 1);
        assert_eq!(update.values[0].property_id, PropertyId::PresentValue);
        assert_eq!(update.values[0].value, ClientDataValue::Real(42.0));

        manager.stop();
        simulator_task.abort();
    }
}
