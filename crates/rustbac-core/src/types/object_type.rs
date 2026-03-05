/// BACnet object type identifiers as defined in the BACnet specification.
///
/// Known standard types are represented as named variants; proprietary
/// vendor-specific types use the [`Proprietary`](Self::Proprietary) variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Parse a BACnet hyphenated object type name (e.g. `"analog-input"`) into an `ObjectType`.
    ///
    /// Returns `None` for unrecognised names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "analog-input" => Some(Self::AnalogInput),
            "analog-output" => Some(Self::AnalogOutput),
            "analog-value" => Some(Self::AnalogValue),
            "binary-input" => Some(Self::BinaryInput),
            "binary-output" => Some(Self::BinaryOutput),
            "binary-value" => Some(Self::BinaryValue),
            "calendar" => Some(Self::Calendar),
            "command" => Some(Self::Command),
            "device" => Some(Self::Device),
            "event-enrollment" => Some(Self::EventEnrollment),
            "file" => Some(Self::File),
            "group" => Some(Self::Group),
            "loop" => Some(Self::Loop),
            "multi-state-input" => Some(Self::MultiStateInput),
            "multi-state-output" => Some(Self::MultiStateOutput),
            "notification-class" => Some(Self::NotificationClass),
            "program" => Some(Self::Program),
            "schedule" => Some(Self::Schedule),
            "averaging" => Some(Self::Averaging),
            "multi-state-value" => Some(Self::MultiStateValue),
            "trend-log" => Some(Self::TrendLog),
            "life-safety-point" => Some(Self::LifeSafetyPoint),
            "life-safety-zone" => Some(Self::LifeSafetyZone),
            "accumulator" => Some(Self::Accumulator),
            "pulse-converter" => Some(Self::PulseConverter),
            "event-log" => Some(Self::EventLog),
            "global-group" => Some(Self::GlobalGroup),
            "trend-log-multiple" => Some(Self::TrendLogMultiple),
            "structured-view" => Some(Self::StructuredView),
            "access-door" => Some(Self::AccessDoor),
            _ => None,
        }
    }
}

impl core::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AnalogInput => f.write_str("analog-input"),
            Self::AnalogOutput => f.write_str("analog-output"),
            Self::AnalogValue => f.write_str("analog-value"),
            Self::BinaryInput => f.write_str("binary-input"),
            Self::BinaryOutput => f.write_str("binary-output"),
            Self::BinaryValue => f.write_str("binary-value"),
            Self::Calendar => f.write_str("calendar"),
            Self::Command => f.write_str("command"),
            Self::Device => f.write_str("device"),
            Self::EventEnrollment => f.write_str("event-enrollment"),
            Self::File => f.write_str("file"),
            Self::Group => f.write_str("group"),
            Self::Loop => f.write_str("loop"),
            Self::MultiStateInput => f.write_str("multi-state-input"),
            Self::MultiStateOutput => f.write_str("multi-state-output"),
            Self::NotificationClass => f.write_str("notification-class"),
            Self::Program => f.write_str("program"),
            Self::Schedule => f.write_str("schedule"),
            Self::Averaging => f.write_str("averaging"),
            Self::MultiStateValue => f.write_str("multi-state-value"),
            Self::TrendLog => f.write_str("trend-log"),
            Self::LifeSafetyPoint => f.write_str("life-safety-point"),
            Self::LifeSafetyZone => f.write_str("life-safety-zone"),
            Self::Accumulator => f.write_str("accumulator"),
            Self::PulseConverter => f.write_str("pulse-converter"),
            Self::EventLog => f.write_str("event-log"),
            Self::GlobalGroup => f.write_str("global-group"),
            Self::TrendLogMultiple => f.write_str("trend-log-multiple"),
            Self::StructuredView => f.write_str("structured-view"),
            Self::AccessDoor => f.write_str("access-door"),
            Self::Proprietary(v) => write!(f, "proprietary-{v}"),
        }
    }
}
