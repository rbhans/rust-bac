use crate::encoding::{
    reader::Reader,
    tag::{AppTag, Tag},
    writer::Writer,
};
use crate::{DecodeError, EncodeError};

pub fn encode_unsigned(w: &mut Writer<'_>, value: u32) -> Result<usize, EncodeError> {
    let len = if value <= 0xFF {
        1
    } else if value <= 0xFFFF {
        2
    } else if value <= 0xFF_FFFF {
        3
    } else {
        4
    };

    for i in (0..len).rev() {
        let b = ((value >> (i * 8)) & 0xFF) as u8;
        w.write_u8(b)?;
    }
    Ok(len)
}

pub fn decode_unsigned(r: &mut Reader<'_>, len: usize) -> Result<u32, DecodeError> {
    if len == 0 || len > 4 {
        return Err(DecodeError::InvalidLength);
    }
    let mut value = 0u32;
    for _ in 0..len {
        value = (value << 8) | r.read_u8()? as u32;
    }
    Ok(value)
}

pub fn encode_signed(w: &mut Writer<'_>, value: i32) -> Result<usize, EncodeError> {
    let value64 = value as i64;
    let len = if (-128..=127).contains(&value64) {
        1
    } else if (-32768..=32767).contains(&value64) {
        2
    } else if (-8_388_608..=8_388_607).contains(&value64) {
        3
    } else {
        4
    };

    let bytes = value.to_be_bytes();
    w.write_all(&bytes[4 - len..])?;
    Ok(len)
}

pub fn decode_signed(r: &mut Reader<'_>, len: usize) -> Result<i32, DecodeError> {
    if len == 0 || len > 4 {
        return Err(DecodeError::InvalidLength);
    }

    let bytes = r.read_exact(len)?;
    let mut out = [0u8; 4];
    out[4 - len..].copy_from_slice(bytes);
    if (bytes[0] & 0x80) != 0 {
        for b in &mut out[..4 - len] {
            *b = 0xFF;
        }
    }
    Ok(i32::from_be_bytes(out))
}

pub fn encode_app_unsigned(w: &mut Writer<'_>, value: u32) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_unsigned(&mut tw, value)? as u32;
    Tag::Application {
        tag: AppTag::UnsignedInt,
        len,
    }
    .encode(w)?;
    w.write_all(&scratch[..len as usize])
}

pub fn encode_app_enumerated(w: &mut Writer<'_>, value: u32) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_unsigned(&mut tw, value)? as u32;
    Tag::Application {
        tag: AppTag::Enumerated,
        len,
    }
    .encode(w)?;
    w.write_all(&scratch[..len as usize])
}

pub fn encode_app_object_id(w: &mut Writer<'_>, object_id_raw: u32) -> Result<(), EncodeError> {
    Tag::Application {
        tag: AppTag::ObjectId,
        len: 4,
    }
    .encode(w)?;
    w.write_be_u32(object_id_raw)
}

pub fn encode_app_signed(w: &mut Writer<'_>, value: i32) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_signed(&mut tw, value)? as u32;
    Tag::Application {
        tag: AppTag::SignedInt,
        len,
    }
    .encode(w)?;
    w.write_all(&scratch[..len as usize])
}

pub fn decode_app_unsigned(r: &mut Reader<'_>) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::UnsignedInt,
            len,
        } => decode_unsigned(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

pub fn decode_app_enumerated(r: &mut Reader<'_>) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::Enumerated,
            len,
        } => decode_unsigned(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

pub fn decode_app_signed(r: &mut Reader<'_>) -> Result<i32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::SignedInt,
            len,
        } => decode_signed(r, len as usize),
        _ => Err(DecodeError::InvalidTag),
    }
}

pub fn encode_ctx_unsigned(w: &mut Writer<'_>, tag_num: u8, value: u32) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_unsigned(&mut tw, value)? as u32;
    Tag::Context { tag_num, len }.encode(w)?;
    w.write_all(&scratch[..len as usize])
}

pub fn encode_ctx_object_id(
    w: &mut Writer<'_>,
    tag_num: u8,
    object_id_raw: u32,
) -> Result<(), EncodeError> {
    Tag::Context { tag_num, len: 4 }.encode(w)?;
    w.write_be_u32(object_id_raw)
}

pub fn encode_ctx_signed(w: &mut Writer<'_>, tag_num: u8, value: i32) -> Result<(), EncodeError> {
    let mut scratch = [0u8; 4];
    let mut tw = Writer::new(&mut scratch);
    let len = encode_signed(&mut tw, value)? as u32;
    Tag::Context { tag_num, len }.encode(w)?;
    w.write_all(&scratch[..len as usize])
}

pub fn encode_ctx_character_string(
    w: &mut Writer<'_>,
    tag_num: u8,
    value: &str,
) -> Result<(), EncodeError> {
    let bytes = value.as_bytes();
    Tag::Context {
        tag_num,
        len: (bytes.len() + 1) as u32,
    }
    .encode(w)?;
    w.write_u8(0)?;
    w.write_all(bytes)
}

pub fn decode_ctx_character_string<'a>(
    r: &mut Reader<'a>,
    len: usize,
) -> Result<&'a str, DecodeError> {
    if len == 0 {
        return Err(DecodeError::InvalidLength);
    }
    let raw = r.read_exact(len)?;
    if raw[0] != 0 {
        return Err(DecodeError::Unsupported);
    }
    core::str::from_utf8(&raw[1..]).map_err(|_| DecodeError::InvalidValue)
}

pub fn encode_opening_tag(w: &mut Writer<'_>, tag_num: u8) -> Result<(), EncodeError> {
    Tag::Opening { tag_num }.encode(w)
}

pub fn encode_closing_tag(w: &mut Writer<'_>, tag_num: u8) -> Result<(), EncodeError> {
    Tag::Closing { tag_num }.encode(w)
}

pub fn encode_app_real(w: &mut Writer<'_>, value: f32) -> Result<(), EncodeError> {
    Tag::Application {
        tag: AppTag::Real,
        len: 4,
    }
    .encode(w)?;
    w.write_all(&value.to_bits().to_be_bytes())
}

pub fn decode_app_real(r: &mut Reader<'_>) -> Result<f32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Application {
            tag: AppTag::Real,
            len: 4,
        } => {
            let bytes = r.read_exact(4)?;
            Ok(f32::from_bits(u32::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ])))
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::{
        decode_app_unsigned, decode_ctx_character_string, decode_unsigned, encode_app_unsigned,
        encode_ctx_character_string, encode_unsigned,
    };
    use crate::encoding::{reader::Reader, writer::Writer};
    use alloc::format;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn unsigned_roundtrip(v in any::<u32>()) {
            let mut b = [0u8; 8];
            let mut w = Writer::new(&mut b);
            let len = encode_unsigned(&mut w, v).unwrap();
            let mut r = Reader::new(w.as_written());
            let got = decode_unsigned(&mut r, len).unwrap();
            prop_assert_eq!(got, v);
        }

        #[test]
        fn app_unsigned_roundtrip(v in any::<u32>()) {
            let mut b = [0u8; 16];
            let mut w = Writer::new(&mut b);
            encode_app_unsigned(&mut w, v).unwrap();
            let mut r = Reader::new(w.as_written());
            let got = decode_app_unsigned(&mut r).unwrap();
            prop_assert_eq!(got, v);
        }

        #[test]
        fn signed_roundtrip(v in any::<i32>()) {
            let mut b = [0u8; 8];
            let mut w = Writer::new(&mut b);
            let len = super::encode_signed(&mut w, v).unwrap();
            let mut r = Reader::new(w.as_written());
            let got = super::decode_signed(&mut r, len).unwrap();
            prop_assert_eq!(got, v);
        }

        #[test]
        fn app_signed_roundtrip(v in any::<i32>()) {
            let mut b = [0u8; 16];
            let mut w = Writer::new(&mut b);
            super::encode_app_signed(&mut w, v).unwrap();
            let mut r = Reader::new(w.as_written());
            let got = super::decode_app_signed(&mut r).unwrap();
            prop_assert_eq!(got, v);
        }
    }

    #[test]
    fn ctx_character_string_roundtrip() {
        let mut b = [0u8; 32];
        let mut w = Writer::new(&mut b);
        encode_ctx_character_string(&mut w, 2, "hello").unwrap();
        let mut r = Reader::new(w.as_written());
        match crate::encoding::tag::Tag::decode(&mut r).unwrap() {
            crate::encoding::tag::Tag::Context { tag_num: 2, len } => {
                let got = decode_ctx_character_string(&mut r, len as usize).unwrap();
                assert_eq!(got, "hello");
            }
            other => panic!("unexpected tag: {other:?}"),
        }
    }
}
