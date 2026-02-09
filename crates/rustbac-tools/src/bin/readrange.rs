use clap::{Parser, ValueEnum};
use rustbac_client::BacnetClient;
use rustbac_core::types::{Date, ObjectId, ObjectType, PropertyId, Time};
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

#[derive(Debug, Clone, ValueEnum)]
enum RangeModeArg {
    Position,
    Sequence,
    Time,
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-readrange")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "trend-log")]
    object_type: ObjectTypeArg,
    #[arg(long)]
    instance: u32,
    #[arg(long, default_value_t = 85)]
    property_id: u32,
    #[arg(long)]
    property_array_index: Option<u32>,
    #[arg(long, value_enum, default_value = "position")]
    mode: RangeModeArg,
    #[arg(long, default_value_t = 1)]
    start_index: i32,
    #[arg(long, default_value_t = 1)]
    start_sequence: u32,
    #[arg(long, default_value_t = 10)]
    count: i16,
    #[arg(long, default_value_t = 126)]
    year_since_1900: u8,
    #[arg(long, default_value_t = 1)]
    month: u8,
    #[arg(long, default_value_t = 1)]
    day: u8,
    #[arg(long, default_value_t = 1)]
    weekday: u8,
    #[arg(long, default_value_t = 0)]
    hour: u8,
    #[arg(long, default_value_t = 0)]
    minute: u8,
    #[arg(long, default_value_t = 0)]
    second: u8,
    #[arg(long, default_value_t = 0)]
    hundredths: u8,
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
    let object_id = ObjectId::new(args.object_type.into_object_type(), args.instance);
    let property_id = PropertyId::from_u32(args.property_id);
    let result = match args.mode {
        RangeModeArg::Position => {
            client
                .read_range_by_position(
                    addr,
                    object_id,
                    property_id,
                    args.property_array_index,
                    args.start_index,
                    args.count,
                )
                .await?
        }
        RangeModeArg::Sequence => {
            client
                .read_range_by_sequence_number(
                    addr,
                    object_id,
                    property_id,
                    args.property_array_index,
                    args.start_sequence,
                    args.count,
                )
                .await?
        }
        RangeModeArg::Time => {
            client
                .read_range_by_time(
                    addr,
                    object_id,
                    property_id,
                    args.property_array_index,
                    (
                        Date {
                            year_since_1900: args.year_since_1900,
                            month: args.month,
                            day: args.day,
                            weekday: args.weekday,
                        },
                        Time {
                            hour: args.hour,
                            minute: args.minute,
                            second: args.second,
                            hundredths: args.hundredths,
                        },
                    ),
                    args.count,
                )
                .await?
        }
    };

    println!("{result:?}");
    Ok(())
}
