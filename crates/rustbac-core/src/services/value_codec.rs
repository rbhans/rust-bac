use crate::encoding::{
    primitives::{decode_signed, decode_unsigned, encode_signed, encode_unsigned},
    reader::Reader,
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::types::{BitString, DataValue, Date, ObjectId, Time};
use crate::{DecodeError, EncodeError};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

fn u32_len(len: usize) -> Result<u32, EncodeError> {
    u32::try_from(len).map_err(|_| EncodeError::ValueOutOfRange)
}

pub fn encode_application_data_value(
    w: &mut Writer<'_>,
    value: &DataValue<'_>,
) -> Result<(), EncodeError> {
    match value {
        DataValue::Null => Tag::Application {
            tag: AppTag::Null,
            len: 0,
        }
        .encode(w),
        DataValue::Boolean(v) => Tag::Application {
            tag: AppTag::Boolean,
            len: if *v { 1 } else { 0 },
        }
        .encode(w),
        DataValue::Unsigned(v) => encode_app_unsigned_like(w, AppTag::UnsignedInt, *v),
        DataValue::Signed(v) => encode_app_signed_like(w, AppTag::SignedInt, *v),
        DataValue::Real(v) => {
            Tag::Application {
                tag: AppTag::Real,
                len: 4,
            }
            .encode(w)?;
            w.write_all(&v.to_bits().to_be_bytes())
        }
        DataValue::Double(v) => {
            Tag::Application {
                tag: AppTag::Double,
                len: 8,
            }
            .encode(w)?;
            w.write_all(&v.to_bits().to_be_bytes())
        }
        DataValue::OctetString(v) => {
            Tag::Application {
                tag: AppTag::OctetString,
                len: u32_len(v.len())?,
            }
            .encode(w)?;
            w.write_all(v)
        }
        DataValue::CharacterString(v) => {
            let bytes = v.as_bytes();
            Tag::Application {
                tag: AppTag::CharacterString,
                len: u32_len(bytes.len().saturating_add(1))?,
            }
            .encode(w)?;
            // BACnet character set 0 = UTF-8/ANSI X3.4 compatible in this baseline.
            w.write_u8(0)?;
            w.write_all(bytes)
        }
        DataValue::BitString(v) => {
            if v.unused_bits > 7 {
                return Err(EncodeError::ValueOutOfRange);
            }
            Tag::Application {
                tag: AppTag::BitString,
                len: u32_len(v.data.len().saturating_add(1))?,
            }
            .encode(w)?;
            w.write_u8(v.unused_bits)?;
            w.write_all(v.data)
        }
        DataValue::Enumerated(v) => encode_app_unsigned_like(w, AppTag::Enumerated, *v),
        DataValue::Date(v) => {
            Tag::Application {
                tag: AppTag::Date,
                len: 4,
            }
            .encode(w)?;
            w.write_all(&[v.year_since_1900, v.month, v.day, v.weekday])
        }
        DataValue::Time(v) => {
            Tag::Application {
                tag: AppTag::Time,
                len: 4,
            }
            .encode(w)?;
            w.write_all(&[v.hour, v.minute, v.second, v.hundredths])
        }
        DataValue::ObjectId(v) => {
            Tag::Application {
                tag: AppTag::ObjectId,
                len: 4,
            }
            .encode(w)?;
            w.write_all(&v.raw().to_be_bytes())
        }
        #[cfg(feature = "alloc")]
        DataValue::Constructed { tag_num, values } => {
            Tag::Opening { tag_num: *tag_num }.encode(w)?;
            for child in values {
                encode_application_data_value(w, child)?;
            }
            Tag::Closing { tag_num: *tag_num }.encode(w)
        }
    }
}

pub fn decode_application_data_value<'a>(r: &mut Reader<'a>) -> Result<DataValue<'a>, DecodeError> {
    let tag = Tag::decode(r)?;
    decode_application_data_value_from_tag(r, tag)
}

pub fn decode_application_data_value_from_tag<'a>(
    r: &mut Reader<'a>,
    tag: Tag,
) -> Result<DataValue<'a>, DecodeError> {
    match tag {
        Tag::Application {
            tag: AppTag::Null, ..
        } => Ok(DataValue::Null),
        Tag::Application {
            tag: AppTag::Boolean,
            len,
        } => Ok(DataValue::Boolean(len != 0)),
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len,
        } => Ok(DataValue::Unsigned(decode_unsigned(r, len as usize)?)),
        Tag::Application {
            tag: AppTag::SignedInt,
            len,
        } => Ok(DataValue::Signed(decode_signed(r, len as usize)?)),
        Tag::Application {
            tag: AppTag::Real,
            len: 4,
        } => {
            let b = r.read_exact(4)?;
            Ok(DataValue::Real(f32::from_bits(u32::from_be_bytes([
                b[0], b[1], b[2], b[3],
            ]))))
        }
        Tag::Application {
            tag: AppTag::Double,
            len: 8,
        } => {
            let b = r.read_exact(8)?;
            Ok(DataValue::Double(f64::from_bits(u64::from_be_bytes([
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            ]))))
        }
        Tag::Application {
            tag: AppTag::OctetString,
            len,
        } => Ok(DataValue::OctetString(r.read_exact(len as usize)?)),
        Tag::Application {
            tag: AppTag::CharacterString,
            len,
        } => {
            if len == 0 {
                return Err(DecodeError::InvalidLength);
            }
            let raw = r.read_exact(len as usize)?;
            let charset = raw[0];
            if charset != 0 {
                return Err(DecodeError::Unsupported);
            }
            let s = core::str::from_utf8(&raw[1..]).map_err(|_| DecodeError::InvalidValue)?;
            Ok(DataValue::CharacterString(s))
        }
        Tag::Application {
            tag: AppTag::BitString,
            len,
        } => {
            if len == 0 {
                return Err(DecodeError::InvalidLength);
            }
            let raw = r.read_exact(len as usize)?;
            if raw[0] > 7 {
                return Err(DecodeError::InvalidValue);
            }
            Ok(DataValue::BitString(BitString {
                unused_bits: raw[0],
                data: &raw[1..],
            }))
        }
        Tag::Application {
            tag: AppTag::Enumerated,
            len,
        } => Ok(DataValue::Enumerated(decode_unsigned(r, len as usize)?)),
        Tag::Application {
            tag: AppTag::Date,
            len: 4,
        } => {
            let b = r.read_exact(4)?;
            Ok(DataValue::Date(Date {
                year_since_1900: b[0],
                month: b[1],
                day: b[2],
                weekday: b[3],
            }))
        }
        Tag::Application {
            tag: AppTag::Time,
            len: 4,
        } => {
            let b = r.read_exact(4)?;
            Ok(DataValue::Time(Time {
                hour: b[0],
                minute: b[1],
                second: b[2],
                hundredths: b[3],
            }))
        }
        Tag::Application {
            tag: AppTag::ObjectId,
            len: 4,
        } => {
            let b = r.read_exact(4)?;
            Ok(DataValue::ObjectId(ObjectId::from_raw(u32::from_be_bytes(
                [b[0], b[1], b[2], b[3]],
            ))))
        }
        #[cfg(feature = "alloc")]
        Tag::Opening { tag_num } => {
            let mut children = Vec::new();
            loop {
                let child_tag = Tag::decode(r)?;
                if child_tag == (Tag::Closing { tag_num }) {
                    break;
                }
                children.push(decode_application_data_value_from_tag(r, child_tag)?);
            }
            Ok(DataValue::Constructed {
                tag_num,
                values: children,
            })
        }
        _ => Err(DecodeError::Unsupported),
    }
}

fn encode_app_unsigned_like(
    w: &mut Writer<'_>,
    tag: AppTag,
    value: u32,
) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_unsigned(&mut tw, value)? as u32;
    Tag::Application { tag, len }.encode(w)?;
    w.write_all(&scratch[..len as usize])
}

fn encode_app_signed_like(w: &mut Writer<'_>, tag: AppTag, value: i32) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_signed(&mut tw, value)? as u32;
    Tag::Application { tag, len }.encode(w)?;
    w.write_all(&scratch[..len as usize])
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::{decode_application_data_value, encode_application_data_value};
    use crate::encoding::{reader::Reader, writer::Writer};
    use crate::types::{BitString, DataValue, Date, ObjectId, ObjectType, Time};

    #[test]
    fn value_codec_roundtrip_supported_types() {
        let values = [
            DataValue::Null,
            DataValue::Boolean(true),
            DataValue::Unsigned(123),
            DataValue::Signed(-123),
            DataValue::Real(12.5),
            DataValue::Double(42.25),
            DataValue::OctetString(&[1, 2, 3]),
            DataValue::CharacterString("hello"),
            DataValue::BitString(BitString::new(1, &[0b1010_0000])),
            DataValue::Enumerated(9),
            DataValue::Date(Date {
                year_since_1900: 124,
                month: 2,
                day: 3,
                weekday: 6,
            }),
            DataValue::Time(Time {
                hour: 1,
                minute: 2,
                second: 3,
                hundredths: 4,
            }),
            DataValue::ObjectId(ObjectId::new(ObjectType::Device, 1)),
        ];

        for v in values {
            let mut buf = [0u8; 64];
            let mut w = Writer::new(&mut buf);
            encode_application_data_value(&mut w, &v).unwrap();
            let mut r = Reader::new(w.as_written());
            let got = decode_application_data_value(&mut r).unwrap();
            assert_eq!(got, v);
        }
    }

    #[test]
    fn value_codec_roundtrip_constructed() {
        use alloc::vec;

        let value = DataValue::Constructed {
            tag_num: 2,
            values: vec![
                DataValue::Unsigned(42),
                DataValue::CharacterString("test"),
                DataValue::Constructed {
                    tag_num: 0,
                    values: vec![DataValue::Boolean(true), DataValue::Real(3.14)],
                },
            ],
        };

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        encode_application_data_value(&mut w, &value).unwrap();
        let mut r = Reader::new(w.as_written());
        let got = decode_application_data_value(&mut r).unwrap();
        assert_eq!(got, value);
    }
}
