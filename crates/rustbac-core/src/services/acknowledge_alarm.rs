use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_character_string, encode_ctx_object_id, encode_ctx_unsigned},
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::types::{Date, ObjectId, Time};
use crate::EncodeError;

pub const SERVICE_ACKNOWLEDGE_ALARM: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EventState {
    Normal = 0,
    Fault = 1,
    Offnormal = 2,
    HighLimit = 3,
    LowLimit = 4,
    LifeSafetyAlarm = 5,
}

impl EventState {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Normal),
            1 => Some(Self::Fault),
            2 => Some(Self::Offnormal),
            3 => Some(Self::HighLimit),
            4 => Some(Self::LowLimit),
            5 => Some(Self::LifeSafetyAlarm),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeStamp {
    Time(Time),
    SequenceNumber(u32),
    DateTime { date: Date, time: Time },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgeAlarmRequest<'a> {
    pub acknowledging_process_id: u32,
    pub event_object_id: ObjectId,
    pub event_state_acknowledged: EventState,
    pub event_time_stamp: TimeStamp,
    pub acknowledgment_source: &'a str,
    pub time_of_acknowledgment: TimeStamp,
    pub invoke_id: u8,
}

impl<'a> AcknowledgeAlarmRequest<'a> {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: self.invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_ACKNOWLEDGE_ALARM,
        }
        .encode(w)?;
        encode_ctx_unsigned(w, 0, self.acknowledging_process_id)?;
        encode_ctx_object_id(w, 1, self.event_object_id.raw())?;
        encode_ctx_unsigned(w, 2, self.event_state_acknowledged.to_u32())?;

        Tag::Opening { tag_num: 3 }.encode(w)?;
        encode_timestamp(w, self.event_time_stamp)?;
        Tag::Closing { tag_num: 3 }.encode(w)?;

        encode_ctx_character_string(w, 4, self.acknowledgment_source)?;

        Tag::Opening { tag_num: 5 }.encode(w)?;
        encode_timestamp(w, self.time_of_acknowledgment)?;
        Tag::Closing { tag_num: 5 }.encode(w)?;
        Ok(())
    }
}

fn encode_timestamp(w: &mut Writer<'_>, value: TimeStamp) -> Result<(), EncodeError> {
    match value {
        TimeStamp::Time(time) => {
            Tag::Context { tag_num: 0, len: 4 }.encode(w)?;
            w.write_all(&[time.hour, time.minute, time.second, time.hundredths])
        }
        TimeStamp::SequenceNumber(seq) => encode_ctx_unsigned(w, 1, seq),
        TimeStamp::DateTime { date, time } => {
            Tag::Opening { tag_num: 2 }.encode(w)?;
            Tag::Application {
                tag: AppTag::Date,
                len: 4,
            }
            .encode(w)?;
            w.write_all(&[date.year_since_1900, date.month, date.day, date.weekday])?;
            Tag::Application {
                tag: AppTag::Time,
                len: 4,
            }
            .encode(w)?;
            w.write_all(&[time.hour, time.minute, time.second, time.hundredths])?;
            Tag::Closing { tag_num: 2 }.encode(w)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AcknowledgeAlarmRequest, EventState, TimeStamp, SERVICE_ACKNOWLEDGE_ALARM};
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{Date, ObjectId, ObjectType, Time};

    #[test]
    fn encode_acknowledge_alarm_request() {
        let req = AcknowledgeAlarmRequest {
            acknowledging_process_id: 12,
            event_object_id: ObjectId::new(ObjectType::AnalogInput, 2),
            event_state_acknowledged: EventState::Offnormal,
            event_time_stamp: TimeStamp::SequenceNumber(42),
            acknowledgment_source: "operator",
            time_of_acknowledgment: TimeStamp::DateTime {
                date: Date {
                    year_since_1900: 126,
                    month: 2,
                    day: 7,
                    weekday: 6,
                },
                time: Time {
                    hour: 10,
                    minute: 11,
                    second: 12,
                    hundredths: 13,
                },
            },
            invoke_id: 9,
        };
        let mut buf = [0u8; 256];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_ACKNOWLEDGE_ALARM);
        assert_eq!(hdr.invoke_id, 9);
    }
}
