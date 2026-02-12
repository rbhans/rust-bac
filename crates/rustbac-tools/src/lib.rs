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
    Command,
    Device,
    EventEnrollment,
    File,
    Group,
    Loop,
    MultiStateInput,
    MultiStateOutput,
    NotificationClass,
    Program,
    Schedule,
    Averaging,
    MultiStateValue,
    TrendLog,
    LifeSafetyPoint,
    LifeSafetyZone,
    Accumulator,
    PulseConverter,
    EventLog,
    GlobalGroup,
    TrendLogMultiple,
    StructuredView,
    AccessDoor,
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
            Self::Command => ObjectType::Command,
            Self::Device => ObjectType::Device,
            Self::EventEnrollment => ObjectType::EventEnrollment,
            Self::File => ObjectType::File,
            Self::Group => ObjectType::Group,
            Self::Loop => ObjectType::Loop,
            Self::MultiStateInput => ObjectType::MultiStateInput,
            Self::MultiStateOutput => ObjectType::MultiStateOutput,
            Self::NotificationClass => ObjectType::NotificationClass,
            Self::Program => ObjectType::Program,
            Self::Schedule => ObjectType::Schedule,
            Self::Averaging => ObjectType::Averaging,
            Self::MultiStateValue => ObjectType::MultiStateValue,
            Self::TrendLog => ObjectType::TrendLog,
            Self::LifeSafetyPoint => ObjectType::LifeSafetyPoint,
            Self::LifeSafetyZone => ObjectType::LifeSafetyZone,
            Self::Accumulator => ObjectType::Accumulator,
            Self::PulseConverter => ObjectType::PulseConverter,
            Self::EventLog => ObjectType::EventLog,
            Self::GlobalGroup => ObjectType::GlobalGroup,
            Self::TrendLogMultiple => ObjectType::TrendLogMultiple,
            Self::StructuredView => ObjectType::StructuredView,
            Self::AccessDoor => ObjectType::AccessDoor,
        }
    }
}
