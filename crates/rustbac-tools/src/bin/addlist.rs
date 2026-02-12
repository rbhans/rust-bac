use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::services::list_element::AddListElementRequest;
use rustbac_core::types::{DataValue, ObjectId, PropertyId};
use rustbac_datalink::DataLinkAddress;
use rustbac_tools::ObjectTypeArg;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-addlist")]
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
        .add_list_element(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            AddListElementRequest {
                object_id: ObjectId::new(args.object_type.into_object_type(), args.instance),
                property_id: PropertyId::from_u32(args.property_id),
                array_index: args.property_array_index,
                elements: &encoded_values,
                invoke_id: 0,
            },
        )
        .await?;
    println!("add-list-element acknowledged");
    Ok(())
}
