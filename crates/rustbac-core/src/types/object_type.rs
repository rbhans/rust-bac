/// BACnet object type identifiers as defined in the BACnet specification.
///
/// Known standard types are represented as named variants; proprietary
/// vendor-specific types use the [`Proprietary`](Self::Proprietary) variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
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
    Proprietary(u16),
}

impl ObjectType {
    /// Converts this object type to its numeric BACnet identifier.
    pub const fn to_u16(self) -> u16 {
        match self {
            Self::AnalogInput => 0,
            Self::AnalogOutput => 1,
            Self::AnalogValue => 2,
            Self::BinaryInput => 3,
            Self::BinaryOutput => 4,
            Self::BinaryValue => 5,
            Self::Calendar => 6,
            Self::Command => 7,
            Self::Device => 8,
            Self::EventEnrollment => 9,
            Self::File => 10,
            Self::Group => 11,
            Self::Loop => 12,
            Self::MultiStateInput => 13,
            Self::MultiStateOutput => 14,
            Self::NotificationClass => 15,
            Self::Program => 16,
            Self::Schedule => 17,
            Self::Averaging => 18,
            Self::MultiStateValue => 19,
            Self::TrendLog => 20,
            Self::LifeSafetyPoint => 21,
            Self::LifeSafetyZone => 22,
            Self::Accumulator => 23,
            Self::PulseConverter => 24,
            Self::EventLog => 25,
            Self::GlobalGroup => 26,
            Self::TrendLogMultiple => 27,
            Self::StructuredView => 29,
            Self::AccessDoor => 30,
            Self::Proprietary(v) => v,
        }
    }

    /// Creates an `ObjectType` from its numeric BACnet identifier.
    ///
    /// Values without a known standard mapping become [`Proprietary`](Self::Proprietary).
    pub const fn from_u16(value: u16) -> Self {
        match value {
            0 => Self::AnalogInput,
            1 => Self::AnalogOutput,
            2 => Self::AnalogValue,
            3 => Self::BinaryInput,
            4 => Self::BinaryOutput,
            5 => Self::BinaryValue,
            6 => Self::Calendar,
            7 => Self::Command,
            8 => Self::Device,
            9 => Self::EventEnrollment,
            10 => Self::File,
            11 => Self::Group,
            12 => Self::Loop,
            13 => Self::MultiStateInput,
            14 => Self::MultiStateOutput,
            15 => Self::NotificationClass,
            16 => Self::Program,
            17 => Self::Schedule,
            18 => Self::Averaging,
            19 => Self::MultiStateValue,
            20 => Self::TrendLog,
            21 => Self::LifeSafetyPoint,
            22 => Self::LifeSafetyZone,
            23 => Self::Accumulator,
            24 => Self::PulseConverter,
            25 => Self::EventLog,
            26 => Self::GlobalGroup,
            27 => Self::TrendLogMultiple,
            29 => Self::StructuredView,
            30 => Self::AccessDoor,
            v => Self::Proprietary(v),
        }
    }
}
