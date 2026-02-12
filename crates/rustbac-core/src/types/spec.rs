/// Segmentation capability advertised during device discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Segmentation {
    SegmentedBoth = 0,
    SegmentedTransmit = 1,
    SegmentedReceive = 2,
    NoSegmentation = 3,
}

/// Maximum APDU length accepted by a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MaxApdu {
    UpTo50 = 0,
    UpTo128 = 1,
    UpTo206 = 2,
    UpTo480 = 3,
    UpTo1024 = 4,
    UpTo1476 = 5,
}

/// BACnet error class reported in Error PDUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorClass {
    Device = 0,
    Object = 1,
    Property = 2,
    Resources = 3,
    Security = 4,
    Services = 5,
    Vt = 6,
    Communication = 7,
}

/// BACnet error code reported in Error PDUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorCode {
    Other = 0,
    DeviceBusy = 3,
    ConfigurationInProgress = 2,
    UnknownObject = 31,
    UnknownProperty = 32,
    WriteAccessDenied = 40,
    ValueOutOfRange = 37,
}

impl Segmentation {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::SegmentedBoth),
            1 => Some(Self::SegmentedTransmit),
            2 => Some(Self::SegmentedReceive),
            3 => Some(Self::NoSegmentation),
            _ => None,
        }
    }
}

impl MaxApdu {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::UpTo50),
            1 => Some(Self::UpTo128),
            2 => Some(Self::UpTo206),
            3 => Some(Self::UpTo480),
            4 => Some(Self::UpTo1024),
            5 => Some(Self::UpTo1476),
            _ => None,
        }
    }
}

impl ErrorClass {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Device),
            1 => Some(Self::Object),
            2 => Some(Self::Property),
            3 => Some(Self::Resources),
            4 => Some(Self::Security),
            5 => Some(Self::Services),
            6 => Some(Self::Vt),
            7 => Some(Self::Communication),
            _ => None,
        }
    }
}

impl ErrorCode {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Other),
            2 => Some(Self::ConfigurationInProgress),
            3 => Some(Self::DeviceBusy),
            31 => Some(Self::UnknownObject),
            32 => Some(Self::UnknownProperty),
            37 => Some(Self::ValueOutOfRange),
            40 => Some(Self::WriteAccessDenied),
            _ => None,
        }
    }
}
