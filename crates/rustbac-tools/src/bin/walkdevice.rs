use clap::Parser;
use rustbac_client::{walk::walk_device, BacnetClient};
use rustbac_core::types::{ObjectId, ObjectType};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-walkdevice")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long)]
    instance: u32,
    #[arg(long)]
    bbmd: Option<SocketAddr>,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
    #[arg(long)]
    json: bool,
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
    let device_id = ObjectId::new(ObjectType::Device, args.instance);

    let result = walk_device(&client, addr, device_id).await;
    match result {
        Ok(walk) => {
            if args.json {
                println!("{}", serde_json::to_string_pretty(&walk)?);
            } else {
                println!(
                    "Device {:?} — {} objects:",
                    walk.device_id,
                    walk.objects.len()
                );
                for obj in &walk.objects {
                    let name = obj.object_name.as_deref().unwrap_or("?");
                    let pv = obj
                        .present_value
                        .as_ref()
                        .map(|v| format!("{v:?}"))
                        .unwrap_or_default();
                    println!("  {:?} \"{name}\" = {pv}", obj.object_id);
                }
            }
        }
        Err(e) => {
            eprintln!("walk failed: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}
