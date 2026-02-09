use clap::Parser;
use rustbac_client::{BacnetClient, BroadcastDistributionEntry};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

fn parse_bdt_entry(value: &str) -> Result<BroadcastDistributionEntry, String> {
    let (addr_part, mask_part) = value
        .split_once('/')
        .ok_or_else(|| "entry must be in ip:port/mask format".to_string())?;
    let address: SocketAddrV4 = addr_part
        .parse()
        .map_err(|e| format!("invalid entry address '{addr_part}': {e}"))?;
    let mask: Ipv4Addr = mask_part
        .parse()
        .map_err(|e| format!("invalid subnet mask '{mask_part}': {e}"))?;
    Ok(BroadcastDistributionEntry { address, mask })
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-write-bdt")]
struct Args {
    #[arg(long)]
    bbmd: SocketAddr,
    #[arg(long, value_parser = parse_bdt_entry, required = true)]
    entry: Vec<BroadcastDistributionEntry>,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let client = BacnetClient::new_foreign(args.bbmd, args.foreign_ttl).await?;
    client
        .write_broadcast_distribution_table(&args.entry)
        .await?;
    println!("wrote {} bdt entries", args.entry.len());
    Ok(())
}
