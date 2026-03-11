//! MS/TP frame encoding and decoding.
//!
//! Implements the frame structure from ASHRAE 135 Clause 9.

use crate::crc;

/// MS/TP preamble bytes.
pub const PREAMBLE: [u8; 2] = [0x55, 0xFF];

/// MS/TP frame types (ASHRAE 135 Clause 9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Token = 0x00,
    PollForMaster = 0x01,
    ReplyToPollForMaster = 0x02,
    TestRequest = 0x03,
    TestResponse = 0x04,
    BacnetDataExpectingReply = 0x05,
    BacnetDataNotExpectingReply = 0x06,
    ReplyPostponed = 0x07,
}

impl FrameType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x00 => Some(Self::Token),
            0x01 => Some(Self::PollForMaster),
            0x02 => Some(Self::ReplyToPollForMaster),
            0x03 => Some(Self::TestRequest),
            0x04 => Some(Self::TestResponse),
            0x05 => Some(Self::BacnetDataExpectingReply),
            0x06 => Some(Self::BacnetDataNotExpectingReply),
            0x07 => Some(Self::ReplyPostponed),
            _ => None,
        }
    }

    /// True if this frame type carries a data payload.
    #[allow(dead_code)]
    pub fn has_data(self) -> bool {
        matches!(
            self,
            Self::TestRequest
                | Self::TestResponse
                | Self::BacnetDataExpectingReply
                | Self::BacnetDataNotExpectingReply
        )
    }
}

/// A decoded MS/TP frame.
#[derive(Debug, Clone)]
pub struct MstpFrame {
    pub frame_type: FrameType,
    pub destination: u8,
    pub source: u8,
    pub data: Vec<u8>,
}

impl MstpFrame {
    /// Encode a frame to bytes (preamble + header + header CRC + data + data CRC).
    pub fn encode(&self) -> Vec<u8> {
        let data_len = self.data.len() as u16;
        let header = [
            self.frame_type as u8,
            self.destination,
            self.source,
            (data_len >> 8) as u8,
            data_len as u8,
        ];
        let header_crc = crc::crc8(&header);

        let mut buf = Vec::with_capacity(8 + self.data.len() + 2);
        buf.extend_from_slice(&PREAMBLE);
        buf.extend_from_slice(&header);
        buf.push(header_crc);

        if !self.data.is_empty() {
            let data_crc = crc::crc16(&self.data);
            buf.extend_from_slice(&self.data);
            buf.extend_from_slice(&data_crc.to_le_bytes());
        }

        buf
    }

    /// Decode a frame from bytes. Expects input starting AFTER the preamble.
    /// Returns `(frame, bytes_consumed)` or `None` if invalid.
    pub fn decode(buf: &[u8]) -> Option<(Self, usize)> {
        if buf.len() < 6 {
            return None; // Need at least header (5 bytes) + header CRC (1 byte)
        }

        // Header: frame_type, dst, src, len_hi, len_lo
        let frame_type = FrameType::from_u8(buf[0])?;
        let destination = buf[1];
        let source = buf[2];
        let data_len = ((buf[3] as u16) << 8) | (buf[4] as u16);

        // Verify header CRC
        if !crc::verify_header_crc(&buf[..6]) {
            return None;
        }

        let mut consumed = 6;

        let data = if data_len > 0 {
            let total_data_bytes = data_len as usize + 2; // data + 2-byte CRC
            if buf.len() < consumed + total_data_bytes {
                return None;
            }
            let data_end = consumed + data_len as usize;
            let crc_end = data_end + 2;

            // Verify data CRC
            if !crc::verify_data_crc(&buf[consumed..crc_end]) {
                return None;
            }

            let data = buf[consumed..data_end].to_vec();
            consumed = crc_end;
            data
        } else {
            Vec::new()
        };

        Some((
            MstpFrame {
                frame_type,
                destination,
                source,
                data,
            },
            consumed,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_token_frame() {
        let frame = MstpFrame {
            frame_type: FrameType::Token,
            destination: 5,
            source: 1,
            data: Vec::new(),
        };
        let encoded = frame.encode();
        assert_eq!(&encoded[..2], &PREAMBLE);
        assert_eq!(encoded[2], 0x00); // Token
        assert_eq!(encoded[3], 5); // dst
        assert_eq!(encoded[4], 1); // src

        let (decoded, consumed) = MstpFrame::decode(&encoded[2..]).unwrap();
        assert_eq!(decoded.frame_type, FrameType::Token);
        assert_eq!(decoded.destination, 5);
        assert_eq!(decoded.source, 1);
        assert!(decoded.data.is_empty());
        assert_eq!(consumed, 6);
    }

    #[test]
    fn encode_decode_data_frame() {
        let payload = vec![0x01, 0x02, 0x03, 0x04];
        let frame = MstpFrame {
            frame_type: FrameType::BacnetDataNotExpectingReply,
            destination: 10,
            source: 3,
            data: payload.clone(),
        };
        let encoded = frame.encode();

        let (decoded, _consumed) = MstpFrame::decode(&encoded[2..]).unwrap();
        assert_eq!(decoded.frame_type, FrameType::BacnetDataNotExpectingReply);
        assert_eq!(decoded.destination, 10);
        assert_eq!(decoded.source, 3);
        assert_eq!(decoded.data, payload);
    }

    #[test]
    fn decode_invalid_crc_returns_none() {
        let frame = MstpFrame {
            frame_type: FrameType::Token,
            destination: 1,
            source: 0,
            data: Vec::new(),
        };
        let mut encoded = frame.encode();
        // Corrupt header CRC
        let last = encoded.len() - 1;
        encoded[last] ^= 0xFF;
        assert!(MstpFrame::decode(&encoded[2..]).is_none());
    }

    #[test]
    fn frame_type_has_data() {
        assert!(!FrameType::Token.has_data());
        assert!(!FrameType::PollForMaster.has_data());
        assert!(FrameType::BacnetDataExpectingReply.has_data());
        assert!(FrameType::BacnetDataNotExpectingReply.has_data());
        assert!(FrameType::TestRequest.has_data());
    }
}
