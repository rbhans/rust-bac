use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::services::write_property::WritePropertyRequest;
use rustbac_core::types::{DataValue, ObjectId, ObjectType, PropertyId};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-writeprop")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long)]
    instance: u32,
    #[arg(long)]
    value: f32,
    #[arg(long)]
    bbmd: Option<SocketAddr>,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let client = match args.bbmd {
        Some(bbmd) => BacnetClient::new_foreign(bbmd, args.foreign_ttl).await?,
        None => BacnetClient::new().await?,
    };
    let addr = DataLinkAddress::Ip((args.ip, args.port).into());

    let req = WritePropertyRequest {
        object_id: ObjectId::new(ObjectType::AnalogOutput, args.instance),
        property_id: PropertyId::PresentValue,
        value: DataValue::Real(args.value),
        priority: Some(8),
        ..Default::default()
    };

    client.write_property(addr, req).await?;
    println!("write request sent");
    Ok(())
}
