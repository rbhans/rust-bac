use clap::{Parser, ValueEnum};
use rustbac_client::BacnetClient;
use rustbac_core::services::list_element::RemoveListElementRequest;
use rustbac_core::types::{DataValue, ObjectId, ObjectType, PropertyId};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

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
#[command(name = "bacnet-removelist")]
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
    property_id: u32,
    #[arg(long)]
    property_array_index: Option<u32>,
    #[arg(long, value_delimiter = ',')]
    values: Vec<u32>,
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
    let encoded_values: Vec<DataValue<'_>> = args
        .values
        .iter()
        .copied()
        .map(DataValue::Unsigned)
        .collect();
    client
        .remove_list_element(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            RemoveListElementRequest {
                object_id: ObjectId::new(args.object_type.into_object_type(), args.instance),
                property_id: PropertyId::from_u32(args.property_id),
                array_index: args.property_array_index,
                elements: &encoded_values,
                invoke_id: 0,
            },
        )
        .await?;
    println!("remove-list-element acknowledged");
    Ok(())
}
