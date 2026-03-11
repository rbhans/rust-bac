//! CRC routines for MS/TP frames (ASHRAE 135 Annex G).
//!
//! Implements the CRC-8 (header) and CRC-16 (data) algorithms specified in
//! ASHRAE 135 Annex G for BACnet MS/TP frames.

/// CRC-8 for MS/TP frame headers.
///
/// Processes a single byte into the running CRC. The polynomial is
/// x^8 + x^2 + x + 1, reflected (0x8C).
fn crc8_byte(crc: u8, data: u8) -> u8 {
    let mut val = crc ^ data;
    for _ in 0..8 {
        if (val & 1) != 0 {
            val = (val >> 1) ^ 0x8C;
        } else {
            val >>= 1;
        }
    }
    val
}

/// Compute the header CRC-8 over the given data.
/// Initial value is 0xFF; the result is ones-complemented.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0xFF;
    for &byte in data {
        crc = crc8_byte(crc, byte);
    }
    !crc
}

/// Verify a header CRC.
///
/// Pass the header bytes followed by the CRC byte. Returns true if valid.
pub fn verify_header_crc(header_and_crc: &[u8]) -> bool {
    let mut crc: u8 = 0xFF;
    for &byte in header_and_crc {
        crc = crc8_byte(crc, byte);
    }
    // After processing header + valid CRC, the accumulator should be
    // the "good CRC" residual for this polynomial.
    crc == HEADER_CRC_GOOD
}

/// CRC-16 for MS/TP frame data.
///
/// Processes a single byte into the running CRC. The polynomial is
/// x^16 + x^15 + x^2 + 1, reflected (0xA001).
fn crc16_byte(crc: u16, data: u8) -> u16 {
    let crc_low = ((crc ^ data as u16) & 0xFF) as u8;
    let mut val = crc_low as u16;
    for _ in 0..8 {
        if (val & 1) != 0 {
            val = (val >> 1) ^ 0xA001;
        } else {
            val >>= 1;
        }
    }
    (crc >> 8) ^ val
}

/// Compute the data CRC-16 over the given data.
/// Initial value is 0xFFFF; the result is ones-complemented.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc = crc16_byte(crc, byte);
    }
    !crc
}

/// Verify a data CRC.
///
/// Pass the data bytes followed by the 2-byte CRC (little-endian). Returns true if valid.
pub fn verify_data_crc(data_and_crc: &[u8]) -> bool {
    let mut crc: u16 = 0xFFFF;
    for &byte in data_and_crc {
        crc = crc16_byte(crc, byte);
    }
    crc == DATA_CRC_GOOD
}

/// The "good CRC" residual for header CRC-8.
/// Computed by running crc8_byte over (header + valid_crc).
const HEADER_CRC_GOOD: u8 = {
    // We derive this by: if the CRC is valid, the accumulator after
    // processing all bytes (including the CRC byte) reaches this constant.
    // For CRC-8 with init=0xFF, poly=0x8C reflected, complement output:
    // The residual when processing data + !accumulator is always the same.
    // We compute it from a trivial case: empty data, crc = !0xFF = 0x00.
    // crc8_byte(0xFF, 0x00):
    let mut val: u8 = 0xFF;
    let mut i = 0;
    while i < 8 {
        if (val & 1) != 0 {
            val = (val >> 1) ^ 0x8C;
        } else {
            val >>= 1;
        }
        i += 1;
    }
    val
    // This gives us the residual constant at compile time.
};

/// The "good CRC" residual for data CRC-16.
const DATA_CRC_GOOD: u16 = {
    // Similar derivation: empty data, crc16 = !0xFFFF = 0x0000 → LE bytes [0x00, 0x00].
    // Process crc16_byte(0xFFFF, 0x00), then crc16_byte(result, 0x00).
    let crc_low_1: u8 = 0xFFFF_u16 as u8; // 0xFF
    let mut val1: u16 = crc_low_1 as u16;
    let mut i = 0;
    while i < 8 {
        if (val1 & 1) != 0 {
            val1 = (val1 >> 1) ^ 0xA001;
        } else {
            val1 >>= 1;
        }
        i += 1;
    }
    let after_first: u16 = (0xFFFF_u16 >> 8) ^ val1;

    let crc_low_2: u8 = (after_first & 0xFF) as u8;
    let mut val2: u16 = crc_low_2 as u16;
    i = 0;
    while i < 8 {
        if (val2 & 1) != 0 {
            val2 = (val2 >> 1) ^ 0xA001;
        } else {
            val2 >>= 1;
        }
        i += 1;
    }
    (after_first >> 8) ^ val2
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc8_roundtrip() {
        let header = [0x00, 0x01, 0x00, 0x00, 0x00];
        let crc = crc8(&header);
        let mut full = header.to_vec();
        full.push(crc);
        assert!(verify_header_crc(&full));
    }

    #[test]
    fn crc8_different_data() {
        let header = [0x01, 0x02, 0x01, 0x00, 0x00];
        let crc = crc8(&header);
        let mut full = header.to_vec();
        full.push(crc);
        assert!(verify_header_crc(&full));
    }

    #[test]
    fn crc16_roundtrip() {
        let data = b"Hello BACnet MS/TP";
        let crc = crc16(data);
        let crc_bytes = crc.to_le_bytes();
        let mut full = data.to_vec();
        full.extend_from_slice(&crc_bytes);
        assert!(verify_data_crc(&full));
    }

    #[test]
    fn crc8_empty() {
        let crc = crc8(&[]);
        assert_eq!(crc, 0x00); // !0xFF = 0x00
    }

    #[test]
    fn crc16_empty() {
        let crc = crc16(&[]);
        assert_eq!(crc, 0x0000); // !0xFFFF = 0x0000
    }

    #[test]
    fn crc8_verify_bad_data() {
        let header = [0x00, 0x01, 0x00, 0x00, 0x00];
        let crc = crc8(&header);
        let mut full = header.to_vec();
        full.push(crc ^ 0xFF); // corrupt CRC
        assert!(!verify_header_crc(&full));
    }

    #[test]
    fn crc16_verify_bad_data() {
        let data = b"test data";
        let crc = crc16(data);
        let mut full = data.to_vec();
        let bad_crc = (crc ^ 0xFFFF).to_le_bytes();
        full.extend_from_slice(&bad_crc);
        assert!(!verify_data_crc(&full));
    }

    #[test]
    fn crc8_deterministic() {
        // Same input always produces same CRC
        let data = [0x05, 0x0A, 0x03, 0x00, 0x10];
        assert_eq!(crc8(&data), crc8(&data));
    }

    #[test]
    fn crc16_deterministic() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        assert_eq!(crc16(&data), crc16(&data));
    }
}
