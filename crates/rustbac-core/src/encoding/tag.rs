use crate::encoding::{reader::Reader, writer::Writer};
use crate::{DecodeError, EncodeError};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTag {
    Null = 0,
    Boolean = 1,
    UnsignedInt = 2,
    SignedInt = 3,
    Real = 4,
    Double = 5,
    OctetString = 6,
    CharacterString = 7,
    BitString = 8,
    Enumerated = 9,
    Date = 10,
    Time = 11,
    ObjectId = 12,
}

impl AppTag {
    pub fn from_u8(value: u8) -> Result<Self, DecodeError> {
        match value {
            0 => Ok(Self::Null),
            1 => Ok(Self::Boolean),
            2 => Ok(Self::UnsignedInt),
            3 => Ok(Self::SignedInt),
            4 => Ok(Self::Real),
            5 => Ok(Self::Double),
            6 => Ok(Self::OctetString),
            7 => Ok(Self::CharacterString),
            8 => Ok(Self::BitString),
            9 => Ok(Self::Enumerated),
            10 => Ok(Self::Date),
            11 => Ok(Self::Time),
            12 => Ok(Self::ObjectId),
            _ => Err(DecodeError::InvalidTag),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Application { tag: AppTag, len: u32 },
    Context { tag_num: u8, len: u32 },
    Opening { tag_num: u8 },
    Closing { tag_num: u8 },
}

impl Tag {
    pub fn encode(self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        match self {
            Tag::Application { tag, len } => encode_with_meta(w, tag as u8, false, len),
            Tag::Context { tag_num, len } => encode_with_meta(w, tag_num, true, len),
            Tag::Opening { tag_num } => encode_open_close(w, tag_num, true),
            Tag::Closing { tag_num } => encode_open_close(w, tag_num, false),
        }
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let first = r.read_u8()?;
        let class_context = (first & 0b0000_1000) != 0;

        let mut tag_num = (first >> 4) & 0x0f;
        if tag_num == 0x0f {
            tag_num = r.read_u8()?;
        }

        let len_val = first & 0x07;

        if class_context && len_val == 6 {
            return Ok(Tag::Opening { tag_num });
        }
        if class_context && len_val == 7 {
            return Ok(Tag::Closing { tag_num });
        }

        let len = decode_len(r, len_val)?;
        if class_context {
            Ok(Tag::Context { tag_num, len })
        } else {
            Ok(Tag::Application {
                tag: AppTag::from_u8(tag_num)?,
                len,
            })
        }
    }
}

fn encode_with_meta(
    w: &mut Writer<'_>,
    tag_num: u8,
    is_context: bool,
    len: u32,
) -> Result<(), EncodeError> {
    let mut first: u8 = 0;

    if tag_num <= 14 {
        first |= tag_num << 4;
    } else {
        first |= 0xF0;
    }

    if is_context {
        first |= 0b0000_1000;
    }

    let len_code = if len <= 4 { len as u8 } else { 5 };

    first |= len_code;
    w.write_u8(first)?;

    if tag_num > 14 {
        w.write_u8(tag_num)?;
    }

    if len_code == 5 {
        if len <= 253 {
            w.write_u8(len as u8)?;
        } else if len <= 65535 {
            w.write_u8(254)?;
            w.write_be_u16(len as u16)?;
        } else {
            w.write_u8(255)?;
            w.write_be_u32(len)?;
        }
    }

    Ok(())
}

fn encode_open_close(w: &mut Writer<'_>, tag_num: u8, opening: bool) -> Result<(), EncodeError> {
    let mut first: u8 = 0b0000_1000;

    if tag_num <= 14 {
        first |= tag_num << 4;
    } else {
        first |= 0xF0;
    }

    first |= if opening { 6 } else { 7 };
    w.write_u8(first)?;

    if tag_num > 14 {
        w.write_u8(tag_num)?;
    }

    Ok(())
}

fn decode_len(r: &mut Reader<'_>, len_code: u8) -> Result<u32, DecodeError> {
    match len_code {
        0..=4 => Ok(len_code as u32),
        5 => {
            let v = r.read_u8()?;
            if v <= 253 {
                Ok(v as u32)
            } else if v == 254 {
                Ok(r.read_be_u16()? as u32)
            } else {
                r.read_be_u32()
            }
        }
        _ => Err(DecodeError::InvalidLength),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppTag, Tag};
    use crate::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn roundtrip_application_tag() {
        let mut buf = [0u8; 8];
        let mut w = Writer::new(&mut buf);
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len: 3,
        }
        .encode(&mut w)
        .unwrap();

        let mut r = Reader::new(w.as_written());
        let t = Tag::decode(&mut r).unwrap();
        assert_eq!(
            t,
            Tag::Application {
                tag: AppTag::UnsignedInt,
                len: 3
            }
        );
    }

    #[test]
    fn roundtrip_extended() {
        let mut buf = [0u8; 16];
        let mut w = Writer::new(&mut buf);
        Tag::Context {
            tag_num: 30,
            len: 300,
        }
        .encode(&mut w)
        .unwrap();

        let mut r = Reader::new(w.as_written());
        let t = Tag::decode(&mut r).unwrap();
        assert_eq!(
            t,
            Tag::Context {
                tag_num: 30,
                len: 300
            }
        );
    }
}
