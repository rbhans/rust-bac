use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::types::ObjectId;
use rustbac_tools::ObjectTypeArg;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "bacnet-whohas")]
struct Args {
    #[arg(long)]
    object_name: Option<String>,
    #[arg(long, value_enum)]
    object_type: Option<ObjectTypeArg>,
    #[arg(long)]
    instance: Option<u32>,
    #[arg(long)]
    low_limit: Option<u32>,
    #[arg(long)]
    high_limit: Option<u32>,
    #[arg(long, default_value_t = 3)]
    wait_seconds: u64,
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
    let range = match (args.low_limit, args.high_limit) {
        (Some(low), Some(high)) => Some((low, high)),
        (None, None) => None,
        _ => {
            return Err("both --low-limit and --high-limit must be set together".into());
        }
    };
    let wait = Duration::from_secs(args.wait_seconds);

    let results = if let Some(object_name) = args.object_name.as_deref() {
        client.who_has_object_name(range, object_name, wait).await?
    } else {
        let object_type = args
            .object_type
            .ok_or("--object-type is required when --object-name is not provided")?;
        let instance = args
            .instance
            .ok_or("--instance is required when --object-name is not provided")?;
        client
            .who_has_object_id(
                range,
                ObjectId::new(object_type.into_object_type(), instance),
                wait,
            )
            .await?
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        for item in &results {
            println!(
                "device={:?} object={:?} name={:?} source={}",
                item.device_id, item.object_id, item.object_name, item.address
            );
        }
        println!("found {} object(s)", results.len());
    }
    Ok(())
}
