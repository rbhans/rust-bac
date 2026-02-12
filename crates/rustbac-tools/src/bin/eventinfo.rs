use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_datalink::DataLinkAddress;
use rustbac_tools::ObjectTypeArg;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-eventinfo")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long)]
    last_object_type: Option<ObjectTypeArg>,
    #[arg(long)]
    last_instance: Option<u32>,
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
    let last_received_object_id = match (args.last_object_type, args.last_instance) {
        (Some(object_type), Some(instance)) => Some(rustbac_core::types::ObjectId::new(
            object_type.into_object_type(),
            instance,
        )),
        (None, None) => None,
        _ => return Err("last_object_type and last_instance must be provided together".into()),
    };
    let result = client
        .get_event_information(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            last_received_object_id,
        )
        .await?;
    println!("{result:?}");
    Ok(())
}
