use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-alarmsummary")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
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

    let summaries = client.get_alarm_summary(addr).await?;
    if summaries.is_empty() {
        println!("no active alarms");
        return Ok(());
    }

    for summary in summaries {
        println!("{summary:?}");
    }
    Ok(())
}
