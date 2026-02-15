/// A BACnet date (year offset from 1900, month, day, weekday).
///
/// A value of `0xFF` in any field means "unspecified".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Date {
    /// Year minus 1900. E.g. `124` means 2024.
    pub year_since_1900: u8,
    /// Month (1–12), or `0xFF` for unspecified.
    pub month: u8,
    /// Day of the month (1–31), or `0xFF` for unspecified.
    pub day: u8,
    /// Day of the week (1 = Monday … 7 = Sunday), or `0xFF` for unspecified.
    pub weekday: u8,
}

/// A BACnet time-of-day with hundredths-of-a-second precision.
///
/// A value of `0xFF` in any field means "unspecified".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub hundredths: u8,
}
