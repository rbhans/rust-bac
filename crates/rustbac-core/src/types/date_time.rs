#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year_since_1900: u8,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub hundredths: u8,
}
