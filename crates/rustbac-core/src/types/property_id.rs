/// BACnet property identifiers.
///
/// Common standard properties are named variants; vendor-specific or
/// unrecognised identifiers use [`Proprietary`](Self::Proprietary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyId {
    ObjectIdentifier,
    ObjectName,
    ObjectType,
    PresentValue,
    Description,
    StatusFlags,
    VendorName,
    Proprietary(u32),
}

impl PropertyId {
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::ObjectIdentifier => 75,
            Self::ObjectName => 77,
            Self::ObjectType => 79,
            Self::PresentValue => 85,
            Self::Description => 28,
            Self::StatusFlags => 111,
            Self::VendorName => 121,
            Self::Proprietary(v) => v,
        }
    }

    pub const fn from_u32(value: u32) -> Self {
        match value {
            75 => Self::ObjectIdentifier,
            77 => Self::ObjectName,
            79 => Self::ObjectType,
            85 => Self::PresentValue,
            28 => Self::Description,
            111 => Self::StatusFlags,
            121 => Self::VendorName,
            v => Self::Proprietary(v),
        }
    }
}
