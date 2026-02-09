use clap::{Parser, ValueEnum};
use rustbac_client::{AtomicReadFileResult, BacnetClient};
use rustbac_core::types::{ObjectId, ObjectType};
use rustbac_datalink::DataLinkAddress;
use std::net::{IpAddr, SocketAddr};

fn to_hex(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[derive(Debug, Clone, ValueEnum)]
enum ModeArg {
    Stream,
    Record,
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-readfile")]
struct Args {
    #[arg(long)]
    ip: IpAddr,
    #[arg(long, default_value_t = 47808)]
    port: u16,
    #[arg(long)]
    instance: u32,
    #[arg(long, value_enum, default_value = "stream")]
    mode: ModeArg,
    #[arg(long, default_value_t = 0)]
    start: i32,
    #[arg(long, default_value_t = 256)]
    count: u32,
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
    let file_object = ObjectId::new(ObjectType::File, args.instance);
    let result = match args.mode {
        ModeArg::Stream => {
            client
                .atomic_read_file_stream(addr, file_object, args.start, args.count)
                .await?
        }
        ModeArg::Record => {
            client
                .atomic_read_file_record(addr, file_object, args.start, args.count)
                .await?
        }
    };

    match result {
        AtomicReadFileResult::Stream {
            end_of_file,
            file_start_position,
            file_data,
        } => {
            println!(
                "mode=stream eof={end_of_file} start={file_start_position} bytes={}",
                file_data.len()
            );
            println!("{}", to_hex(&file_data));
        }
        AtomicReadFileResult::Record {
            end_of_file,
            file_start_record,
            returned_record_count,
            file_record_data,
        } => {
            println!(
                "mode=record eof={end_of_file} start_record={file_start_record} returned={returned_record_count}"
            );
            for (idx, record) in file_record_data.iter().enumerate() {
                println!("record[{idx}] {}", to_hex(record));
            }
        }
    }
    Ok(())
}
