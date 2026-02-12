use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::services::subscribe_cov::SubscribeCovRequest;
use rustbac_core::types::ObjectId;
use rustbac_datalink::DataLinkAddress;
use rustbac_tools::ObjectTypeArg;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "bacnet-listen")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    /// Subscribe to COV for this object type.
    #[arg(long, value_enum)]
    object_type: Option<ObjectTypeArg>,
    /// Instance number for COV subscription.
    #[arg(long)]
    instance: Option<u32>,
    #[arg(long, default_value_t = 1)]
    process_id: u32,
    #[arg(long, default_value_t = 0)]
    lifetime_seconds: u32,
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

    // Optionally subscribe to COV for a specific object.
    if let (Some(object_type), Some(instance)) = (args.object_type, args.instance) {
        let object_id = ObjectId::new(object_type.into_object_type(), instance);
        let lifetime = if args.lifetime_seconds == 0 {
            None
        } else {
            Some(args.lifetime_seconds)
        };
        client
            .subscribe_cov(
                addr,
                SubscribeCovRequest {
                    subscriber_process_id: args.process_id,
                    monitored_object_id: object_id,
                    issue_confirmed_notifications: Some(true),
                    lifetime_seconds: lifetime,
                    invoke_id: 0,
                },
            )
            .await?;
        println!("COV subscription active for {object_id:?}");
    }

    println!("Listening for notifications (Ctrl+C to stop)...");
    let poll_interval = Duration::from_secs(1);
    loop {
        if let Some(cov) = client.recv_cov_notification(poll_interval).await? {
            println!(
                "COV: object={:?} values={:?}",
                cov.monitored_object_id, cov.values
            );
        }
        if let Some(evt) = client.recv_event_notification(poll_interval).await? {
            println!(
                "EVENT: object={:?} type={} state={:?}->{:?}",
                evt.event_object_id, evt.event_type, evt.from_state, evt.to_state
            );
        }
    }
}
