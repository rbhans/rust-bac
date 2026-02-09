use crate::apdu::UnconfirmedRequestHeader;
use crate::encoding::{
    reader::Reader,
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::types::{Date, Time};
use crate::{DecodeError, EncodeError};

pub const SERVICE_TIME_SYNCHRONIZATION: u8 = 0x06;
pub const SERVICE_UTC_TIME_SYNCHRONIZATION: u8 = 0x09;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSynchronizationRequest {
    pub date: Date,
    pub time: Time,
    pub utc: bool,
}

impl TimeSynchronizationRequest {
    pub const fn local(date: Date, time: Time) -> Self {
        Self {
            date,
            time,
            utc: false,
        }
    }

    pub const fn utc(date: Date, time: Time) -> Self {
        Self {
            date,
            time,
            utc: true,
        }
    }

    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        let service_choice = if self.utc {
            SERVICE_UTC_TIME_SYNCHRONIZATION
        } else {
            SERVICE_TIME_SYNCHRONIZATION
        };
        UnconfirmedRequestHeader { service_choice }.encode(w)?;
        Tag::Application {
            tag: AppTag::Date,
            len: 4,
        }
        .encode(w)?;
        w.write_all(&[
            self.date.year_since_1900,
            self.date.month,
            self.date.day,
            self.date.weekday,
        ])?;
        Tag::Application {
            tag: AppTag::Time,
            len: 4,
        }
        .encode(w)?;
        w.write_all(&[
            self.time.hour,
            self.time.minute,
            self.time.second,
            self.time.hundredths,
        ])?;
        Ok(())
    }

    pub fn decode_after_header(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let date = match Tag::decode(r)? {
            Tag::Application {
                tag: AppTag::Date,
                len: 4,
            } => {
                let bytes = r.read_exact(4)?;
                Date {
                    year_since_1900: bytes[0],
                    month: bytes[1],
                    day: bytes[2],
                    weekday: bytes[3],
                }
            }
            _ => return Err(DecodeError::InvalidTag),
        };
        let time = match Tag::decode(r)? {
            Tag::Application {
                tag: AppTag::Time,
                len: 4,
            } => {
                let bytes = r.read_exact(4)?;
                Time {
                    hour: bytes[0],
                    minute: bytes[1],
                    second: bytes[2],
                    hundredths: bytes[3],
                }
            }
            _ => return Err(DecodeError::InvalidTag),
        };
        Ok(Self {
            date,
            time,
            utc: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TimeSynchronizationRequest, SERVICE_TIME_SYNCHRONIZATION, SERVICE_UTC_TIME_SYNCHRONIZATION,
    };
    use crate::apdu::UnconfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{Date, Time};

    #[test]
    fn encode_local_time_sync_request() {
        let req = TimeSynchronizationRequest::local(
            Date {
                year_since_1900: 126,
                month: 2,
                day: 7,
                weekday: 6,
            },
            Time {
                hour: 10,
                minute: 11,
                second: 12,
                hundredths: 13,
            },
        );
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_TIME_SYNCHRONIZATION);
    }

    #[test]
    fn encode_utc_time_sync_request() {
        let req = TimeSynchronizationRequest::utc(
            Date {
                year_since_1900: 126,
                month: 2,
                day: 7,
                weekday: 6,
            },
            Time {
                hour: 14,
                minute: 15,
                second: 16,
                hundredths: 17,
            },
        );
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_UTC_TIME_SYNCHRONIZATION);
    }

    #[test]
    fn decode_local_time_sync_after_header() {
        let req = TimeSynchronizationRequest::local(
            Date {
                year_since_1900: 126,
                month: 2,
                day: 7,
                weekday: 6,
            },
            Time {
                hour: 10,
                minute: 11,
                second: 12,
                hundredths: 13,
            },
        );
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let _hdr = UnconfirmedRequestHeader::decode(&mut r).unwrap();
        let decoded = TimeSynchronizationRequest::decode_after_header(&mut r).unwrap();
        assert_eq!(decoded.date, req.date);
        assert_eq!(decoded.time, req.time);
    }
}
