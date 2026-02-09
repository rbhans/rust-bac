use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::types::{ObjectId, ObjectType, PropertyId};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-readprop")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, default_value_t = 0)]
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
    let addr = DataLinkAddress::Ip((args.ip, args.port).into());
    let result = client
        .read_property(
            addr,
            ObjectId::new(ObjectType::Device, args.instance),
            PropertyId::ObjectName,
        )
        .await;

    match result {
        Ok(v) => println!("value: {v:?}"),
        Err(e) => {
            eprintln!("read failed: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}
