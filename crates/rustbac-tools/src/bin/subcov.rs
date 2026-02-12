use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::services::subscribe_cov::SubscribeCovRequest;
use rustbac_core::services::subscribe_cov_property::SubscribeCovPropertyRequest;
use rustbac_core::types::{ObjectId, PropertyId};
use rustbac_datalink::DataLinkAddress;
use rustbac_tools::ObjectTypeArg;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "bacnet-subcov")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long, value_enum, default_value = "analog-input")]
    object_type: ObjectTypeArg,
    #[arg(long)]
    instance: u32,
    #[arg(long, default_value_t = 1)]
    process_id: u32,
    #[arg(long, default_value_t = 600)]
    lifetime_seconds: u32,
    #[arg(long)]
    property_id: Option<u32>,
    #[arg(long)]
    property_array_index: Option<u32>,
    #[arg(long)]
    cov_increment: Option<f32>,
    #[arg(long)]
    confirmed_notifications: bool,
    #[arg(long)]
    cancel: bool,
    #[arg(long, default_value_t = 0)]
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

    let addr = DataLinkAddress::Ip((args.ip, args.port).into());
    let object_id = ObjectId::new(args.object_type.into_object_type(), args.instance);

    if args.cancel {
        if let Some(property_id) = args.property_id {
            client
                .cancel_cov_property_subscription(
                    addr,
                    args.process_id,
                    object_id,
                    PropertyId::from_u32(property_id),
                    args.property_array_index,
                )
                .await?;
        } else {
            client
                .cancel_cov_subscription(addr, args.process_id, object_id)
                .await?;
        }
        println!("COV subscription canceled");
        return Ok(());
    }

    if let Some(property_id) = args.property_id {
        client
            .subscribe_cov_property(
                addr,
                SubscribeCovPropertyRequest {
                    subscriber_process_id: args.process_id,
                    monitored_object_id: object_id,
                    issue_confirmed_notifications: Some(args.confirmed_notifications),
                    lifetime_seconds: Some(args.lifetime_seconds),
                    monitored_property_id: PropertyId::from_u32(property_id),
                    monitored_property_array_index: args.property_array_index,
                    cov_increment: args.cov_increment,
                    invoke_id: 0,
                },
            )
            .await?;
    } else {
        client
            .subscribe_cov(
                addr,
                SubscribeCovRequest {
                    subscriber_process_id: args.process_id,
                    monitored_object_id: object_id,
                    issue_confirmed_notifications: Some(args.confirmed_notifications),
                    lifetime_seconds: Some(args.lifetime_seconds),
                    invoke_id: 0,
                },
            )
            .await?;
    }

    println!(
        "COV subscription active: process_id={} object={:?} lifetime={}s confirmed={}",
        args.process_id, object_id, args.lifetime_seconds, args.confirmed_notifications
    );

    if args.listen_seconds > 0 {
        println!(
            "Listening for COV notifications for {} seconds...",
            args.listen_seconds
        );
        let deadline = Instant::now() + Duration::from_secs(args.listen_seconds);
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = remaining.min(Duration::from_secs(1));
            if let Some(notification) = client.recv_cov_notification(wait).await? {
                println!("{notification:?}");
            }
        }
    }

    Ok(())
}
