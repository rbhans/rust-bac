use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_datalink::DataLinkAddress;
use rustbac_tools::ObjectTypeArg;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-createobj")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "analog-value")]
    object_type: ObjectTypeArg,
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
    let object_id = client
        .create_object_by_type(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            args.object_type.into_object_type(),
        )
        .await?;
    println!("created object: {:?}", object_id);
    Ok(())
}
