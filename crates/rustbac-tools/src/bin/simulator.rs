use clap::Parser;
use rustbac_client::{ClientDataValue, SimulatedDevice};
use rustbac_core::types::{ObjectId, ObjectType, PropertyId};
use rustbac_datalink::bip::transport::BacnetIpTransport;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "bacnet-simulator")]
struct Args {
    /// Device instance number.
    #[arg(long, default_value_t = 9999)]
    instance: u32,
    /// Number of analog-input objects to create.
    #[arg(long, default_value_t = 3)]
    analog_inputs: u32,
    /// Number of binary-input objects to create.
    #[arg(long, default_value_t = 2)]
    binary_inputs: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let bind_addr: std::net::SocketAddr = "0.0.0.0:47808".parse()?;
    let transport = BacnetIpTransport::bind(bind_addr).await?;
    let sim = SimulatedDevice::new(args.instance, transport);

    for i in 0..args.analog_inputs {
        let oid = ObjectId::new(ObjectType::AnalogInput, i);
        let mut props = HashMap::new();
        props.insert(
            PropertyId::ObjectName,
            ClientDataValue::CharacterString(format!("AI-{i}")),
        );
        props.insert(PropertyId::PresentValue, ClientDataValue::Real(0.0));
        props.insert(
            PropertyId::ObjectType,
            ClientDataValue::Enumerated(ObjectType::AnalogInput.to_u16() as u32),
        );
        sim.add_object(oid, props).await;
    }

    for i in 0..args.binary_inputs {
        let oid = ObjectId::new(ObjectType::BinaryInput, i);
        let mut props = HashMap::new();
        props.insert(
            PropertyId::ObjectName,
            ClientDataValue::CharacterString(format!("BI-{i}")),
        );
        props.insert(PropertyId::PresentValue, ClientDataValue::Enumerated(0));
        props.insert(
            PropertyId::ObjectType,
            ClientDataValue::Enumerated(ObjectType::BinaryInput.to_u16() as u32),
        );
        sim.add_object(oid, props).await;
    }

    println!(
        "Simulated device {} running ({} AI, {} BI). Ctrl+C to stop.",
        args.instance, args.analog_inputs, args.binary_inputs
    );
    sim.run().await?;
    Ok(())
}
