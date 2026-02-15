use clap::Parser;
use rustbac_client::BacnetClient;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "bacnet-whois")]
struct Args {
    #[arg(long, default_value_t = 3)]
    timeout_secs: u64,
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
    let devices = client
        .who_is(None, Duration::from_secs(args.timeout_secs))
        .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
    } else {
        for (i, d) in devices.iter().enumerate() {
            println!("{i}: {}", d.address);
        }
    }
    Ok(())
}
