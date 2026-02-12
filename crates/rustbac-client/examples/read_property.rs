//! Read a BACnet property from a device.
//!
//! Usage:
//!   cargo run -p rustbac-client --example read_property -- --ip 192.168.1.100

use rustbac_client::BacnetClient;
use rustbac_core::types::{ObjectId, ObjectType, PropertyId};
use rustbac_datalink::DataLinkAddress;
use std::net::IpAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let ip: IpAddr = std::env::args()
        .skip_while(|a| a != "--ip")
        .nth(1)
        .expect("usage: --ip <device-ip>")
        .parse()?;

    // Create a BACnet/IP client bound to the default UDP port (47808).
    let client = BacnetClient::new().await?;

    // Read the object-name of device instance 1.
    let addr = DataLinkAddress::Ip((ip, 47808).into());
    let object_id = ObjectId::new(ObjectType::Device, 1);

    let value = client
        .read_property(addr, object_id, PropertyId::ObjectName)
        .await?;

    println!("Device object-name: {value:?}");
    Ok(())
}
