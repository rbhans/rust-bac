use clap::Parser;
use rustbac_client::BacnetClient;
use std::net::{IpAddr, SocketAddr, SocketAddrV4};

#[derive(Parser, Debug)]
#[command(name = "bacnet-delete-fdt")]
struct Args {
    #[arg(long)]
    bbmd: SocketAddr,
    #[arg(long)]
    target_ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    target_port: u16,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let client = BacnetClient::new_foreign(args.bbmd, args.foreign_ttl).await?;
    let IpAddr::V4(target_ip) = args.target_ip else {
        return Err("target_ip must be IPv4".into());
    };
    client
        .delete_foreign_device_table_entry(SocketAddrV4::new(target_ip, args.target_port))
        .await?;
    println!("deleted fdt entry {}:{}", target_ip, args.target_port);
    Ok(())
}
