use clap::{Parser, ValueEnum};
use rustbac_client::{BacnetClient, DeviceCommunicationState};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, ValueEnum)]
enum StateArg {
    Enable,
    Disable,
    DisableInitiation,
}

impl StateArg {
    const fn into_state(self) -> DeviceCommunicationState {
        match self {
            Self::Enable => DeviceCommunicationState::Enable,
            Self::Disable => DeviceCommunicationState::Disable,
            Self::DisableInitiation => DeviceCommunicationState::DisableInitiation,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-dcc")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "disable")]
    state: StateArg,
    #[arg(long)]
    duration_seconds: Option<u16>,
    #[arg(long)]
    password: Option<String>,
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
        .device_communication_control(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            args.duration_seconds,
            args.state.into_state(),
            args.password.as_deref(),
        )
        .await?;
    println!("device communication control request acknowledged");
    Ok(())
}
