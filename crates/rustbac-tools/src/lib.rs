use clap::ValueEnum;
use rustbac_core::types::ObjectType;

/// CLI-friendly enum for selecting BACnet object types.
///
/// Maps human-readable names to [`ObjectType`] variants for use with clap argument parsing.
#[derive(Debug, Clone, ValueEnum)]
pub enum ObjectTypeArg {
    AnalogInput,
    AnalogOutput,
    AnalogValue,
    BinaryInput,
    BinaryOutput,
    BinaryValue,
    Calendar,
    Device,
    EventEnrollment,
    File,
    NotificationClass,
    Schedule,
    TrendLog,
    MultiStateInput,
    MultiStateOutput,
    MultiStateValue,
}

impl ObjectTypeArg {
    /// Convert to the core [`ObjectType`] representation.
    pub const fn into_object_type(self) -> ObjectType {
        match self {
            Self::AnalogInput => ObjectType::AnalogInput,
            Self::AnalogOutput => ObjectType::AnalogOutput,
            Self::AnalogValue => ObjectType::AnalogValue,
            Self::BinaryInput => ObjectType::BinaryInput,
            Self::BinaryOutput => ObjectType::BinaryOutput,
            Self::BinaryValue => ObjectType::BinaryValue,
            Self::Calendar => ObjectType::Calendar,
            Self::Device => ObjectType::Device,
            Self::EventEnrollment => ObjectType::EventEnrollment,
            Self::File => ObjectType::File,
            Self::NotificationClass => ObjectType::NotificationClass,
            Self::Schedule => ObjectType::Schedule,
            Self::TrendLog => ObjectType::TrendLog,
            Self::MultiStateInput => ObjectType::MultiStateInput,
            Self::MultiStateOutput => ObjectType::MultiStateOutput,
            Self::MultiStateValue => ObjectType::MultiStateValue,
        }
    }
}
