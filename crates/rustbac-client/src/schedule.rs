//! Convenience types and helpers for BACnet Schedule and Calendar objects.
//!
//! Provides typed representations of weekly schedules, exception schedules,
//! and calendar entries that wrap the lower-level [`ClientDataValue`] encoding.

use crate::ClientDataValue;
use rustbac_core::types::{Date, Time};

/// A single time-value pair in a daily schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeValue {
    pub time: Time,
    pub value: ClientDataValue,
}

/// A date range for exception schedules and calendar entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub start: Date,
    pub end: Date,
}

/// An entry in a BACnet Calendar's date-list property.
#[derive(Debug, Clone, PartialEq)]
pub enum CalendarEntry {
    Date(Date),
    Range(DateRange),
    WeekNDay {
        month: u8,
        week_of_month: u8,
        day_of_week: u8,
    },
}

/// Decode a weekly schedule from a [`ClientDataValue::Constructed`].
///
/// A BACnet weekly schedule is a sequence of 7 daily schedules (Sun–Sat),
/// each containing a list of [`TimeValue`] pairs.
pub fn decode_weekly_schedule(value: &ClientDataValue) -> Option<Vec<Vec<TimeValue>>> {
    let days = match value {
        ClientDataValue::Constructed { values, .. } => values,
        _ => return None,
    };

    let mut week = Vec::with_capacity(7);
    for day in days {
        let day_values = match day {
            ClientDataValue::Constructed { values, .. } => values,
            _ => {
                week.push(Vec::new());
                continue;
            }
        };

        let mut entries = Vec::new();
        let mut i = 0;
        while i + 1 < day_values.len() {
            if let ClientDataValue::Time(t) = &day_values[i] {
                entries.push(TimeValue {
                    time: *t,
                    value: day_values[i + 1].clone(),
                });
                i += 2;
            } else {
                i += 1;
            }
        }
        week.push(entries);
    }

    Some(week)
}

/// Encode a weekly schedule into a [`ClientDataValue::Constructed`].
pub fn encode_weekly_schedule(week: &[Vec<TimeValue>]) -> ClientDataValue {
    let mut days = Vec::with_capacity(week.len());
    for (i, day) in week.iter().enumerate() {
        let mut values = Vec::with_capacity(day.len() * 2);
        for entry in day {
            values.push(ClientDataValue::Time(entry.time));
            values.push(entry.value.clone());
        }
        days.push(ClientDataValue::Constructed {
            tag_num: i as u8,
            values,
        });
    }
    ClientDataValue::Constructed {
        tag_num: 0,
        values: days,
    }
}

/// Decode a date-list from a [`ClientDataValue::Constructed`] into calendar entries.
pub fn decode_date_list(value: &ClientDataValue) -> Option<Vec<CalendarEntry>> {
    let items = match value {
        ClientDataValue::Constructed { values, .. } => values,
        _ => return None,
    };

    let mut entries = Vec::new();
    for item in items {
        match item {
            ClientDataValue::Date(d) => entries.push(CalendarEntry::Date(*d)),
            ClientDataValue::Constructed { tag_num: 1, values } if values.len() == 2 => {
                if let (ClientDataValue::Date(start), ClientDataValue::Date(end)) =
                    (&values[0], &values[1])
                {
                    entries.push(CalendarEntry::Range(DateRange {
                        start: *start,
                        end: *end,
                    }));
                }
            }
            ClientDataValue::Constructed { tag_num: 2, values } if values.len() == 3 => {
                if let (
                    ClientDataValue::Unsigned(month),
                    ClientDataValue::Unsigned(week),
                    ClientDataValue::Unsigned(day),
                ) = (&values[0], &values[1], &values[2])
                {
                    entries.push(CalendarEntry::WeekNDay {
                        month: *month as u8,
                        week_of_month: *week as u8,
                        day_of_week: *day as u8,
                    });
                }
            }
            _ => {}
        }
    }

    Some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustbac_core::types::Time;

    #[test]
    fn weekly_schedule_roundtrip() {
        let monday = vec![
            TimeValue {
                time: Time {
                    hour: 8,
                    minute: 0,
                    second: 0,
                    hundredths: 0,
                },
                value: ClientDataValue::Real(72.0),
            },
            TimeValue {
                time: Time {
                    hour: 18,
                    minute: 0,
                    second: 0,
                    hundredths: 0,
                },
                value: ClientDataValue::Real(65.0),
            },
        ];

        let mut week = vec![Vec::new(); 7];
        week[1] = monday.clone();

        let encoded = encode_weekly_schedule(&week);
        let decoded = decode_weekly_schedule(&encoded).unwrap();
        assert_eq!(decoded.len(), 7);
        assert!(decoded[0].is_empty());
        assert_eq!(decoded[1].len(), 2);
        assert_eq!(decoded[1][0].time.hour, 8);
        assert_eq!(decoded[1][1].time.hour, 18);
    }

    #[test]
    fn decode_date_list_entries() {
        let date = Date {
            year_since_1900: 124,
            month: 12,
            day: 25,
            weekday: 0xFF,
        };

        let value = ClientDataValue::Constructed {
            tag_num: 0,
            values: vec![ClientDataValue::Date(date)],
        };

        let entries = decode_date_list(&value).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], CalendarEntry::Date(date));
    }
}
