use clap::Parser;
use rustbac_client::BacnetClient;
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(name = "bacnet-read-fdt")]
struct Args {
    #[arg(long)]
    bbmd: SocketAddr,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let client = BacnetClient::new_foreign(args.bbmd, args.foreign_ttl).await?;
    let entries = client.read_foreign_device_table().await?;
    if entries.is_empty() {
        println!("fdt is empty");
        return Ok(());
    }
    for entry in entries {
        println!("{entry:?}");
    }
    Ok(())
}
