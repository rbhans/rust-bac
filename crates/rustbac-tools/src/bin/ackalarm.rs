use clap::{Parser, ValueEnum};
use rustbac_client::{BacnetClient, EventState, TimeStamp};
use rustbac_core::services::acknowledge_alarm::AcknowledgeAlarmRequest;
use rustbac_core::types::{ObjectId, ObjectType};
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
enum EventStateArg {
    Normal,
    Fault,
    Offnormal,
    HighLimit,
    LowLimit,
    LifeSafetyAlarm,
}

impl EventStateArg {
    const fn into_event_state(self) -> EventState {
        match self {
            Self::Normal => EventState::Normal,
            Self::Fault => EventState::Fault,
            Self::Offnormal => EventState::Offnormal,
            Self::HighLimit => EventState::HighLimit,
            Self::LowLimit => EventState::LowLimit,
            Self::LifeSafetyAlarm => EventState::LifeSafetyAlarm,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-ackalarm")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "analog-input")]
    object_type: ObjectTypeArg,
    #[arg(long)]
    instance: u32,
    #[arg(long, default_value_t = 1)]
    process_id: u32,
    #[arg(long, value_enum, default_value = "offnormal")]
    event_state: EventStateArg,
    #[arg(long, default_value_t = 0)]
    event_sequence: u32,
    #[arg(long, default_value_t = 0)]
    ack_sequence: u32,
    #[arg(long, default_value = "operator")]
    source: String,
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

    client
        .acknowledge_alarm(
            addr,
            AcknowledgeAlarmRequest {
                acknowledging_process_id: args.process_id,
                event_object_id: ObjectId::new(args.object_type.into_object_type(), args.instance),
                event_state_acknowledged: args.event_state.into_event_state(),
                event_time_stamp: TimeStamp::SequenceNumber(args.event_sequence),
                acknowledgment_source: &args.source,
                time_of_acknowledgment: TimeStamp::SequenceNumber(args.ack_sequence),
                invoke_id: 0,
            },
        )
        .await?;
    println!("acknowledge-alarm request acknowledged");
    Ok(())
}
