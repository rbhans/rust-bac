use clap::{Parser, ValueEnum};
use rustbac_client::{AtomicWriteFileResult, BacnetClient};
use rustbac_core::types::{ObjectId, ObjectType};
use rustbac_datalink::DataLinkAddress;
use std::fs;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, ValueEnum)]
enum ModeArg {
    Stream,
    Record,
}

#[derive(Parser, Debug)]
#[command(name = "bacnet-writefile")]
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
    #[arg(long)]
    data_hex: Option<String>,
    #[arg(long)]
    data_file: Option<String>,
    #[arg(long)]
    bbmd: Option<SocketAddr>,
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,
}

fn decode_hex(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = input.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err("hex data must have even length".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for i in (0..bytes.len()).step_by(2) {
        let hex = std::str::from_utf8(&bytes[i..i + 2])?;
        out.push(u8::from_str_radix(hex, 16)?);
    }
    Ok(out)
}

fn load_payload(args: &Args) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match (&args.data_hex, &args.data_file) {
        (Some(hex), None) => decode_hex(hex),
        (None, Some(path)) => Ok(fs::read(path)?),
        (Some(_), Some(_)) => Err("use either --data-hex or --data-file, not both".into()),
        (None, None) => Err("missing payload: provide --data-hex or --data-file".into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let payload = load_payload(&args)?;
    let client = match args.bbmd {
        Some(bbmd) => BacnetClient::new_foreign(bbmd, args.foreign_ttl).await?,
        None => BacnetClient::new().await?,
    };
    let addr = DataLinkAddress::Ip((args.ip, args.port).into());
    let file_object = ObjectId::new(ObjectType::File, args.instance);

    let result = match args.mode {
        ModeArg::Stream => {
            client
                .atomic_write_file_stream(addr, file_object, args.start, &payload)
                .await?
        }
        ModeArg::Record => {
            let single = [&payload[..]];
            client
                .atomic_write_file_record(addr, file_object, args.start, &single)
                .await?
        }
    };

    match result {
        AtomicWriteFileResult::Stream {
            file_start_position,
        } => {
            println!("write stream acknowledged at start_position={file_start_position}");
        }
        AtomicWriteFileResult::Record { file_start_record } => {
            println!("write record acknowledged at start_record={file_start_record}");
        }
    }
    Ok(())
}
