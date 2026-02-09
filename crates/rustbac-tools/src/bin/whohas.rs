use clap::{Parser, ValueEnum};
use rustbac_client::BacnetClient;
use rustbac_core::types::{ObjectId, ObjectType};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone, ValueEnum)]
enum ObjectTypeArg {
    AnalogInput,
    AnalogOutput,
    AnalogValue,
    BinaryInput,
    BinaryOutput,
    BinaryValue,
    Device,
    File,
    TrendLog,
    MultiStateInput,
    MultiStateOutput,
    MultiStateValue,
}

impl ObjectTypeArg {
    const fn into_object_type(self) -> ObjectType {
        match self {
            Self::AnalogInput => ObjectType::AnalogInput,
            Self::AnalogOutput => ObjectType::AnalogOutput,
            Self::AnalogValue => ObjectType::AnalogValue,
            Self::BinaryInput => ObjectType::BinaryInput,
            Self::BinaryOutput => ObjectType::BinaryOutput,
            Self::BinaryValue => ObjectType::BinaryValue,
            Self::Device => ObjectType::Device,
            Self::File => ObjectType::File,
            Self::TrendLog => ObjectType::TrendLog,
            Self::MultiStateInput => ObjectType::MultiStateInput,
            Self::MultiStateOutput => ObjectType::MultiStateOutput,
            Self::MultiStateValue => ObjectType::MultiStateValue,
        }
    }
}

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

    for item in &results {
        println!(
            "device={:?} object={:?} name={:?} source={}",
            item.device_id, item.object_id, item.object_name, item.address
        );
    }
    println!("found {} object(s)", results.len());
    Ok(())
}
