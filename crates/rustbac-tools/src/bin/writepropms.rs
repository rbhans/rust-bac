use clap::Parser;
use rustbac_client::BacnetClient;
use rustbac_core::services::write_property_multiple::PropertyWriteSpec;
use rustbac_core::types::{DataValue, ObjectId, ObjectType, PropertyId};
use rustbac_datalink::DataLinkAddress;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

/// Parse an ObjectType from a name like "analog-input" or a numeric string like "1".
fn parse_object_type(s: &str) -> Result<ObjectType, String> {
    // Try numeric first
    if let Ok(n) = s.parse::<u16>() {
        return Ok(ObjectType::from_u16(n));
    }
    // Try name (case-insensitive, accept kebab-case or PascalCase)
    let normalized = s.to_ascii_lowercase().replace(['-', '_'], "");
    let ot = match normalized.as_str() {
        "analoginput" => ObjectType::AnalogInput,
        "analogoutput" => ObjectType::AnalogOutput,
        "analogvalue" => ObjectType::AnalogValue,
        "binaryinput" => ObjectType::BinaryInput,
        "binaryoutput" => ObjectType::BinaryOutput,
        "binaryvalue" => ObjectType::BinaryValue,
        "calendar" => ObjectType::Calendar,
        "command" => ObjectType::Command,
        "device" => ObjectType::Device,
        "eventenrollment" => ObjectType::EventEnrollment,
        "file" => ObjectType::File,
        "group" => ObjectType::Group,
        "loop" => ObjectType::Loop,
        "multistateinput" => ObjectType::MultiStateInput,
        "multistateoutput" => ObjectType::MultiStateOutput,
        "notificationclass" => ObjectType::NotificationClass,
        "program" => ObjectType::Program,
        "schedule" => ObjectType::Schedule,
        "averaging" => ObjectType::Averaging,
        "multistatevalue" => ObjectType::MultiStateValue,
        "trendlog" => ObjectType::TrendLog,
        "lifesafetypoint" => ObjectType::LifeSafetyPoint,
        "lifesafetyzone" => ObjectType::LifeSafetyZone,
        "accumulator" => ObjectType::Accumulator,
        "pulseconverter" => ObjectType::PulseConverter,
        "eventlog" => ObjectType::EventLog,
        "globalgroup" => ObjectType::GlobalGroup,
        "trendlogmultiple" => ObjectType::TrendLogMultiple,
        "structuredview" => ObjectType::StructuredView,
        "accessdoor" => ObjectType::AccessDoor,
        _ => return Err(format!("unknown object type: {s:?}")),
    };
    Ok(ot)
}

/// Parse a PropertyId from a name like "present-value" or a numeric string like "85".
fn parse_property_id(s: &str) -> Result<PropertyId, String> {
    if let Ok(n) = s.parse::<u32>() {
        return Ok(PropertyId::from_u32(n));
    }
    let normalized = s.to_ascii_lowercase().replace(['-', '_'], "");
    let pid = match normalized.as_str() {
        "ackedtransitions" => PropertyId::AckedTransitions,
        "activetext" => PropertyId::ActiveText,
        "apdutimeout" => PropertyId::ApduTimeout,
        "applicationsoftwareversion" => PropertyId::ApplicationSoftwareVersion,
        "buffersize" => PropertyId::BufferSize,
        "covincrement" => PropertyId::CovIncrement,
        "databaserevision" => PropertyId::DatabaseRevision,
        "datelist" => PropertyId::DateList,
        "deadband" => PropertyId::Deadband,
        "description" => PropertyId::Description,
        "effectiveperiod" => PropertyId::EffectivePeriod,
        "enable" => PropertyId::Enable,
        "eventenable" => PropertyId::EventEnable,
        "eventstate" => PropertyId::EventState,
        "eventtimestamps" => PropertyId::EventTimeStamps,
        "exceptionschedule" => PropertyId::ExceptionSchedule,
        "firmwarerevision" => PropertyId::FirmwareRevision,
        "highlimit" => PropertyId::HighLimit,
        "inactivetext" => PropertyId::InactiveText,
        "limitenable" => PropertyId::LimitEnable,
        "listofobjectpropertyreferences" => PropertyId::ListOfObjectPropertyReferences,
        "logbuffer" => PropertyId::LogBuffer,
        "logdeviceobjectproperty" => PropertyId::LogDeviceObjectProperty,
        "loginterval" => PropertyId::LogInterval,
        "lowlimit" => PropertyId::LowLimit,
        "maxapdulengthaccepted" => PropertyId::MaxApduLengthAccepted,
        "maxpresvalue" => PropertyId::MaxPresValue,
        "minpresvalue" => PropertyId::MinPresValue,
        "modelname" => PropertyId::ModelName,
        "notificationclass" => PropertyId::NotificationClass,
        "notifytype" => PropertyId::NotifyType,
        "numberofapduretries" => PropertyId::NumberOfApduRetries,
        "objectidentifier" => PropertyId::ObjectIdentifier,
        "objectlist" => PropertyId::ObjectList,
        "objectname" => PropertyId::ObjectName,
        "objecttype" => PropertyId::ObjectType,
        "outofservice" => PropertyId::OutOfService,
        "presentvalue" => PropertyId::PresentValue,
        "priorityarray" => PropertyId::PriorityArray,
        "protocolrevision" => PropertyId::ProtocolRevision,
        "protocolversion" => PropertyId::ProtocolVersion,
        "recipientlist" => PropertyId::RecipientList,
        "recordcount" => PropertyId::RecordCount,
        "reliability" => PropertyId::Reliability,
        "relinquishdefault" => PropertyId::RelinquishDefault,
        "resolution" => PropertyId::Resolution,
        "scheduledefault" => PropertyId::ScheduleDefault,
        "segmentationsupported" => PropertyId::SegmentationSupported,
        "starttime" => PropertyId::StartTime,
        "statusflags" => PropertyId::StatusFlags,
        "stoptime" => PropertyId::StopTime,
        "systemstatus" => PropertyId::SystemStatus,
        "timedelay" => PropertyId::TimeDelay,
        "totalrecordcount" => PropertyId::TotalRecordCount,
        "units" => PropertyId::Units,
        "updateinterval" => PropertyId::UpdateInterval,
        "vendorname" => PropertyId::VendorName,
        "weeklyschedule" => PropertyId::WeeklySchedule,
        _ => return Err(format!("unknown property id: {s:?}")),
    };
    Ok(pid)
}

/// Auto-detect or explicitly type a DataValue from a string.
///
/// With `--type float|unsigned|boolean|string` the value is coerced to that
/// BACnet application type.  Without `--type` the following heuristics apply:
///   - "true" / "false"  → Boolean
///   - parseable as integer → Unsigned
///   - parseable as float   → Real
///   - anything else        → CharacterString
fn parse_value<'a>(raw: &'a str, type_hint: Option<&str>) -> Result<DataValue<'a>, String> {
    match type_hint {
        Some("float") | Some("real") => raw
            .parse::<f32>()
            .map(DataValue::Real)
            .map_err(|_| format!("cannot parse {raw:?} as float")),
        Some("unsigned") | Some("uint") => raw
            .parse::<u32>()
            .map(DataValue::Unsigned)
            .map_err(|_| format!("cannot parse {raw:?} as unsigned integer")),
        Some("boolean") | Some("bool") => match raw.to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(DataValue::Boolean(true)),
            "false" | "0" => Ok(DataValue::Boolean(false)),
            _ => Err(format!("cannot parse {raw:?} as boolean")),
        },
        Some("string") | Some("charstring") => Ok(DataValue::CharacterString(raw)),
        Some(other) => Err(format!("unknown type hint: {other:?}")),
        None => {
            if raw.eq_ignore_ascii_case("true") {
                return Ok(DataValue::Boolean(true));
            }
            if raw.eq_ignore_ascii_case("false") {
                return Ok(DataValue::Boolean(false));
            }
            if let Ok(u) = raw.parse::<u32>() {
                return Ok(DataValue::Unsigned(u));
            }
            if let Ok(f) = raw.parse::<f32>() {
                return Ok(DataValue::Real(f));
            }
            Ok(DataValue::CharacterString(raw))
        }
    }
}

/// A single property-write specification on the command line.
///
/// Each `--write` argument takes three positional values:
///   OBJECT_TYPE:INSTANCE  PROPERTY  VALUE
///
/// Example: `--write analog-output:1 present-value 72.5`
#[derive(Debug, Clone)]
struct WriteSpec {
    object_type: ObjectType,
    instance: u32,
    property: PropertyId,
    value_raw: String,
}

/// Parse a raw `--write` token triplet into a `WriteSpec`.
///
/// clap will hand us a single string containing all three space-separated
/// tokens because we use `num_args(3)`.  Actually with `num_args(3)` clap
/// collects them as a `Vec<String>` of three items.
fn parse_write_arg(parts: &[String]) -> Result<WriteSpec, String> {
    if parts.len() != 3 {
        return Err(format!(
            "--write requires exactly 3 arguments: OBJECT_TYPE:INSTANCE PROPERTY VALUE, got {}",
            parts.len()
        ));
    }
    let obj_str = &parts[0];
    let prop_str = &parts[1];
    let val_str = parts[2].clone();

    // Split "analog-output:1" into type + instance
    let (type_part, inst_part) = obj_str
        .rsplit_once(':')
        .ok_or_else(|| format!("expected OBJECT_TYPE:INSTANCE, got {obj_str:?}"))?;

    let object_type = parse_object_type(type_part)?;
    let instance: u32 = inst_part
        .parse()
        .map_err(|_| format!("expected numeric instance, got {inst_part:?}"))?;
    let property = parse_property_id(prop_str)?;

    Ok(WriteSpec {
        object_type,
        instance,
        property,
        value_raw: val_str,
    })
}

#[derive(Parser, Debug)]
#[command(
    name = "bacnet-writepropms",
    about = "BACnet WritePropertyMultiple — write several properties in one request"
)]
struct Args {
    /// Target device IP address
    #[arg(long)]
    ip: IpAddr,

    /// Target device UDP port (default: 47808)
    #[arg(long, default_value_t = 47808)]
    port: u16,

    /// BBMD address for foreign-device registration (IP:PORT)
    #[arg(long)]
    bbmd: Option<SocketAddr>,

    /// Foreign-device TTL in seconds (used only with --bbmd)
    #[arg(long, default_value_t = 60)]
    foreign_ttl: u16,

    /// Device instance (unused for routing; kept for parity with other tools)
    #[arg(long)]
    instance: Option<u32>,

    /// Write priority (1–16); omit to use the device default
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=16))]
    priority: Option<u8>,

    /// Force a specific BACnet application data type for every value.
    /// Accepted: float, unsigned, boolean, string
    #[arg(long, value_name = "TYPE")]
    r#type: Option<String>,

    /// One or more property writes: OBJECT_TYPE:INSTANCE PROPERTY VALUE
    ///
    /// May be repeated to write multiple properties (to one or more objects).
    ///
    /// Examples:
    ///   --write analog-output:1 present-value 72.5
    ///   --write binary-value:3 out-of-service true
    #[arg(long = "write", value_names = ["OBJECT_TYPE:INSTANCE", "PROPERTY", "VALUE"], num_args = 3, action = clap::ArgAction::Append)]
    writes_raw: Vec<String>,

    /// Output result as JSON
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();

    if args.writes_raw.is_empty() {
        eprintln!("error: at least one --write OBJECT_TYPE:INSTANCE PROPERTY VALUE is required");
        std::process::exit(1);
    }

    // args.writes_raw is a flat Vec<String> of length N*3 because clap
    // appends all num_args(3) tokens into one vec when using Append.
    if args.writes_raw.len() % 3 != 0 {
        eprintln!(
            "error: --write requires exactly 3 arguments each; got {} tokens total",
            args.writes_raw.len()
        );
        std::process::exit(1);
    }

    // Parse write specs
    let mut specs: Vec<WriteSpec> = Vec::new();
    for chunk in args.writes_raw.chunks(3) {
        match parse_write_arg(chunk) {
            Ok(s) => specs.push(s),
            Err(e) => {
                if args.json {
                    println!("{}", serde_json::json!({ "ok": false, "error": e }));
                } else {
                    eprintln!("error: {e}");
                }
                std::process::exit(1);
            }
        }
    }

    // Build the client
    let client = match args.bbmd {
        Some(bbmd) => BacnetClient::new_foreign(bbmd, args.foreign_ttl).await?,
        None => BacnetClient::new().await?,
    };
    let addr = DataLinkAddress::Ip((args.ip, args.port).into());

    // Group specs by ObjectId so we can pass them to write_property_multiple
    // (which targets a single object per call).
    // Use a Vec to preserve insertion order while still grouping by object.
    let mut object_order: Vec<(u16, u32)> = Vec::new();
    let mut grouped: HashMap<(u16, u32), Vec<WriteSpec>> = HashMap::new();

    for spec in specs {
        let key = (spec.object_type.to_u16(), spec.instance);
        if !grouped.contains_key(&key) {
            object_order.push(key);
        }
        grouped.entry(key).or_default().push(spec);
    }

    // Perform one write_property_multiple call per unique object.
    for key in &object_order {
        let group = &grouped[key];
        let object_id = ObjectId::new(ObjectType::from_u16(key.0), key.1);

        // We need DataValue to live as long as the slice we pass to the
        // client.  Build owned values first, then reference them.
        let mut property_values: Vec<(PropertyId, DataValue<'_>, Option<u8>)> = Vec::new();
        for spec in group {
            let dv = match parse_value(&spec.value_raw, args.r#type.as_deref()) {
                Ok(v) => v,
                Err(e) => {
                    if args.json {
                        println!("{}", serde_json::json!({ "ok": false, "error": e }));
                    } else {
                        eprintln!("error: {e}");
                    }
                    std::process::exit(1);
                }
            };
            property_values.push((spec.property, dv, args.priority));
        }

        let write_specs: Vec<PropertyWriteSpec<'_>> = property_values
            .iter()
            .map(|(pid, dv, prio)| PropertyWriteSpec {
                property_id: *pid,
                array_index: None,
                value: dv.clone(),
                priority: *prio,
            })
            .collect();

        let result = client
            .write_property_multiple(addr, object_id, &write_specs)
            .await;

        match result {
            Ok(()) => {
                if args.json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "object_type": key.0,
                            "instance": key.1,
                        })
                    );
                } else {
                    println!("OK: {:?} instance {}", ObjectType::from_u16(key.0), key.1);
                }
            }
            Err(e) => {
                if args.json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": false,
                            "object_type": key.0,
                            "instance": key.1,
                            "error": e.to_string(),
                        })
                    );
                } else {
                    eprintln!(
                        "error writing {:?} instance {}: {e}",
                        ObjectType::from_u16(key.0),
                        key.1
                    );
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
