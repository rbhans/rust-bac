use clap::{Parser, ValueEnum};
use rustbac_client::{BacnetClient, ReinitializeState};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, ValueEnum)]
enum ReinitStateArg {
    Coldstart,
    Warmstart,
    StartBackup,
    EndBackup,
    StartRestore,
    EndRestore,
    AbortRestore,
    ActivateChanges,
}

impl ReinitStateArg {
    const fn into_state(self) -> ReinitializeState {
        match self {
            Self::Coldstart => ReinitializeState::Coldstart,
            Self::Warmstart => ReinitializeState::Warmstart,
            Self::StartBackup => ReinitializeState::StartBackup,
            Self::EndBackup => ReinitializeState::EndBackup,
            Self::StartRestore => ReinitializeState::StartRestore,
            Self::EndRestore => ReinitializeState::EndRestore,
            Self::AbortRestore => ReinitializeState::AbortRestore,
            Self::ActivateChanges => ReinitializeState::ActivateChanges,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-reinit")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "warmstart")]
    state: ReinitStateArg,
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
        .reinitialize_device(
            DataLinkAddress::Ip((args.ip, args.port).into()),
            args.state.into_state(),
            args.password.as_deref(),
        )
        .await?;
    println!("reinitialize-device request acknowledged");
    Ok(())
}
