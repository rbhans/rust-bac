use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(name = "bacnet-privatetransfer")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long)]
    vendor_id: u32,
    #[arg(long)]
    service_number: u32,
    /// Comma-separated decimal byte values for service parameters (e.g. "1,2,3,4").
    #[arg(long)]
    params: Option<String>,
    #[arg(long)]
    bbmd: Option<SocketAddr>,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
}

fn parse_byte_list(s: &str) -> Result<Vec<u8>, String> {
    s.split(',')
        .map(|b| {
            b.trim()
                .parse::<u8>()
                .map_err(|e| format!("invalid byte '{b}': {e}"))
        })
        .collect()
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

    let params = args.params.as_deref().map(parse_byte_list).transpose()?;

    let result = client
        .private_transfer(addr, args.vendor_id, args.service_number, params.as_deref())
        .await;

    match result {
        Ok(ack) => {
            println!("vendor_id: {}", ack.vendor_id);
            println!("service_number: {}", ack.service_number);
            if let Some(block) = &ack.result_block {
                println!("result_block: {block:?}");
            }
        }
        Err(e) => {
            eprintln!("private transfer failed: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}
