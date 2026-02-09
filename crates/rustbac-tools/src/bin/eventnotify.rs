use clap::Parser;
use rustbac_client::BacnetClient;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "bacnet-eventnotify")]
struct Args {
    #[arg(long, default_value_t = 30)]
    listen_seconds: u64,
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

    let deadline = Instant::now() + Duration::from_secs(args.listen_seconds.max(1));
    println!(
        "Listening for BACnet event notifications for {} seconds...",
        args.listen_seconds.max(1)
    );
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_secs(1));
        if let Some(notification) = client.recv_event_notification(wait).await? {
            println!("{notification:?}");
        }
    }

    Ok(())
}
