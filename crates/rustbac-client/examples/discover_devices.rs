//! Discover BACnet devices on the local network using Who-Is.
//!
//! Usage:
//!   cargo run -p rustbac-client --example discover_devices

use rustbac_client::BacnetClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create a BACnet/IP client bound to the default UDP port (47808).
    let client = BacnetClient::new().await?;

    // Broadcast a Who-Is and collect I-Am responses for 3 seconds.
    let devices = client.who_is(None, Duration::from_secs(3)).await?;

    if devices.is_empty() {
        println!("No devices found.");
    } else {
        for device in &devices {
            println!("Device {:?} at {}", device.device_id, device.address,);
        }
        println!("\nDiscovered {} device(s).", devices.len());
    }

    Ok(())
}
