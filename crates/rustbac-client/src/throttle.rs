use rustbac_datalink::DataLinkAddress;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

#[derive(Clone, Copy, Debug)]
struct DeviceThrottleConfig {
    max_concurrent: usize,
    min_interval: Duration,
}

/// Per-device request coordination primitive.
///
/// This utility lets callers limit concurrent requests per target address and
/// enforce a minimum delay between request starts.
#[derive(Debug)]
pub struct DeviceThrottle {
    semaphores: Mutex<HashMap<DataLinkAddress, Arc<Semaphore>>>,
    last_request: Mutex<HashMap<DataLinkAddress, Instant>>,
    overrides: Mutex<HashMap<DataLinkAddress, DeviceThrottleConfig>>,
    default_max_concurrent: usize,
    default_min_interval: Duration,
}

impl DeviceThrottle {
    /// Creates a new throttle using the given defaults.
    pub fn new(max_concurrent: usize, min_interval: Duration) -> Self {
        Self {
            semaphores: Mutex::new(HashMap::new()),
            last_request: Mutex::new(HashMap::new()),
            overrides: Mutex::new(HashMap::new()),
            default_max_concurrent: max_concurrent.max(1),
            default_min_interval: min_interval,
        }
    }

    /// Sets (or replaces) a per-device override.
    pub async fn set_device_limit(
        &self,
        address: DataLinkAddress,
        max_concurrent: usize,
        min_interval: Duration,
    ) {
        let max_concurrent = max_concurrent.max(1);
        self.overrides.lock().await.insert(
            address,
            DeviceThrottleConfig {
                max_concurrent,
                min_interval,
            },
        );

        // Swap in a new semaphore so future acquisitions use the new limit.
        self.semaphores
            .lock()
            .await
            .insert(address, Arc::new(Semaphore::new(max_concurrent)));
    }

    /// Acquires a permit for `address`, respecting per-device concurrency and
    /// minimum interval between request starts.
    pub async fn acquire(&self, address: DataLinkAddress) -> OwnedSemaphorePermit {
        let config = {
            let overrides = self.overrides.lock().await;
            overrides
                .get(&address)
                .copied()
                .unwrap_or(DeviceThrottleConfig {
                    max_concurrent: self.default_max_concurrent,
                    min_interval: self.default_min_interval,
                })
        };

        let semaphore = {
            let mut semaphores = self.semaphores.lock().await;
            semaphores
                .entry(address)
                .or_insert_with(|| Arc::new(Semaphore::new(config.max_concurrent)))
                .clone()
        };

        let permit = semaphore
            .acquire_owned()
            .await
            .expect("device throttle semaphore closed unexpectedly");

        if !config.min_interval.is_zero() {
            let mut last_request = self.last_request.lock().await;
            if let Some(last) = last_request.get(&address) {
                let elapsed = last.elapsed();
                if elapsed < config.min_interval {
                    tokio::time::sleep(config.min_interval - elapsed).await;
                }
            }
            last_request.insert(address, Instant::now());
        }

        permit
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceThrottle;
    use rustbac_datalink::DataLinkAddress;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;
    use tokio::time::{timeout, Instant};

    fn addr(port: u16) -> DataLinkAddress {
        DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
    }

    #[tokio::test]
    async fn enforces_concurrency_limit() {
        let throttle = DeviceThrottle::new(1, Duration::ZERO);
        let first = throttle.acquire(addr(47808)).await;

        let blocked = timeout(Duration::from_millis(40), throttle.acquire(addr(47808))).await;
        assert!(blocked.is_err());

        drop(first);
        let second = timeout(Duration::from_millis(200), throttle.acquire(addr(47808)))
            .await
            .expect("second permit should be acquired");
        drop(second);
    }

    #[tokio::test]
    async fn enforces_minimum_interval() {
        let throttle = DeviceThrottle::new(1, Duration::from_millis(80));
        let first = throttle.acquire(addr(47809)).await;
        drop(first);

        let started = Instant::now();
        let second = throttle.acquire(addr(47809)).await;
        let elapsed = started.elapsed();
        drop(second);

        assert!(
            elapsed >= Duration::from_millis(70),
            "elapsed {:?} was shorter than expected interval",
            elapsed
        );
    }

    #[tokio::test]
    async fn applies_per_device_overrides() {
        let throttle = DeviceThrottle::new(1, Duration::from_millis(120));
        let target = addr(47810);
        let other = addr(47811);

        throttle
            .set_device_limit(target, 2, Duration::from_millis(10))
            .await;

        let first = throttle.acquire(target).await;
        let second = throttle.acquire(target).await;
        let third = timeout(Duration::from_millis(40), throttle.acquire(target)).await;
        assert!(
            third.is_err(),
            "third permit should block at override limit"
        );
        drop(first);
        drop(second);

        let first_other = throttle.acquire(other).await;
        let blocked_other = timeout(Duration::from_millis(40), throttle.acquire(other)).await;
        assert!(blocked_other.is_err(), "default limit should still be one");
        drop(first_other);
    }
}
