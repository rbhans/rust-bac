use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::types::{Date, Time};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-timesync")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
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
    utc: bool,
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
        .time_synchronize(
            DataLinkAddress::Ip((args.ip, args.port).into()),
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
            args.utc,
        )
        .await?;
    println!("time synchronization request sent");
    Ok(())
}
