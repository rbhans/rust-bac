use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::types::ObjectId;
use rustbac_datalink::DataLinkAddress;
use rustbac_tools::ObjectTypeArg;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-deleteobj")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "analog-value")]
    object_type: ObjectTypeArg,
    #[arg(long)]
    instance: u32,
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
    client
        .delete_object(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            ObjectId::new(args.object_type.into_object_type(), args.instance),
        )
        .await?;
    println!("delete object acknowledged");
    Ok(())
}
