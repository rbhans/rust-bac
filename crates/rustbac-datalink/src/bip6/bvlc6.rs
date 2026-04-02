//! BACnet/IPv6 (Annex U) BVLC header encoding and decoding.
//!
//! The BIP6 BVLC uses type byte `0x82` to distinguish from BIP's `0x81`.

use rustbac_core::encoding::{reader::Reader, writer::Writer};
use rustbac_core::{DecodeError, EncodeError};

/// BVLC type byte for BACnet/IPv6.
pub const BVLC_TYPE_BIP6: u8 = 0x82;

/// BIP6 BVLC function codes (ASHRAE 135 Annex U).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bvlc6Function {
    /// BVLC-Result (0x00).
    Result,
    /// Original-Unicast-NPDU (0x01).
    OriginalUnicastNpdu,
    /// Original-Broadcast-NPDU (0x02).
    OriginalBroadcastNpdu,
    /// Address-Resolution (0x03).
    AddressResolution,
    /// Forwarded-Address-Resolution (0x04).
    ForwardedAddressResolution,
    /// Address-Resolution-Ack (0x05).
    AddressResolutionAck,
    /// Virtual-Address-Resolution (0x06).
    VirtualAddressResolution,
    /// Virtual-Address-Resolution-Ack (0x07).
    VirtualAddressResolutionAck,
    /// Forwarded-NPDU (0x08).
    ForwardedNpdu,
    /// Register-Foreign-Device (0x09).
    RegisterForeignDevice,
    /// Delete-Foreign-Device-Table-Entry (0x0A).
    DeleteForeignDeviceTableEntry,
    /// Secure-BVLL (0x0B).
    SecureBvll,
    /// Distribute-Broadcast-To-Network (0x0C).
    DistributeBroadcastToNetwork,
    /// Unknown function code.
    Unknown(u8),
}

impl Bvlc6Function {
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0x00 => Self::Result,
            0x01 => Self::OriginalUnicastNpdu,
            0x02 => Self::OriginalBroadcastNpdu,
            0x03 => Self::AddressResolution,
            0x04 => Self::ForwardedAddressResolution,
            0x05 => Self::AddressResolutionAck,
            0x06 => Self::VirtualAddressResolution,
            0x07 => Self::VirtualAddressResolutionAck,
            0x08 => Self::ForwardedNpdu,
            0x09 => Self::RegisterForeignDevice,
            0x0A => Self::DeleteForeignDeviceTableEntry,
            0x0B => Self::SecureBvll,
            0x0C => Self::DistributeBroadcastToNetwork,
            v => Self::Unknown(v),
        }
    }

    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Result => 0x00,
            Self::OriginalUnicastNpdu => 0x01,
            Self::OriginalBroadcastNpdu => 0x02,
            Self::AddressResolution => 0x03,
            Self::ForwardedAddressResolution => 0x04,
            Self::AddressResolutionAck => 0x05,
            Self::VirtualAddressResolution => 0x06,
            Self::VirtualAddressResolutionAck => 0x07,
            Self::ForwardedNpdu => 0x08,
            Self::RegisterForeignDevice => 0x09,
            Self::DeleteForeignDeviceTableEntry => 0x0A,
            Self::SecureBvll => 0x0B,
            Self::DistributeBroadcastToNetwork => 0x0C,
            Self::Unknown(v) => v,
        }
    }
}

/// BIP6 BVLC header.
///
/// Unlike BIP (Annex J), the BIP6 header includes a 3-byte source virtual address
/// field in addition to the function code and length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bvlc6Header {
    pub function: Bvlc6Function,
    /// Total message length including the BVLC header itself (minimum 10).
    pub length: u16,
    /// Source virtual address (3 bytes — device address within the BACnet/IPv6 network).
    pub source_virtual_address: [u8; 3],
}

impl Bvlc6Header {
    /// Encode the header to the writer.
    ///
    /// Wire format: `[0x82] [function:1] [length:2BE] [src_vaddr:3]` = 7 bytes minimum.
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.write_u8(BVLC_TYPE_BIP6)?;
        w.write_u8(self.function.to_u8())?;
        w.write_be_u16(self.length)?;
        w.write_all(&self.source_virtual_address)
    }

    /// Decode the header from the reader.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let type_byte = r.read_u8()?;
        if type_byte != BVLC_TYPE_BIP6 {
            return Err(DecodeError::InvalidValue);
        }
        let function = Bvlc6Function::from_u8(r.read_u8()?);
        let length = r.read_be_u16()?;
        if length < 7 {
            return Err(DecodeError::InvalidLength);
        }
        let vaddr_bytes = r.read_exact(3)?;
        let mut source_virtual_address = [0u8; 3];
        source_virtual_address.copy_from_slice(vaddr_bytes);
        Ok(Self {
            function,
            length,
            source_virtual_address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustbac_core::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn bvlc6_roundtrip() {
        let h = Bvlc6Header {
            function: Bvlc6Function::OriginalUnicastNpdu,
            length: 20,
            source_virtual_address: [0x00, 0x01, 0x02],
        };
        let mut buf = [0u8; 16];
        let mut w = Writer::new(&mut buf);
        h.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let decoded = Bvlc6Header::decode(&mut r).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn bvlc6_broadcast_roundtrip() {
        let h = Bvlc6Header {
            function: Bvlc6Function::OriginalBroadcastNpdu,
            length: 30,
            source_virtual_address: [0xAA, 0xBB, 0xCC],
        };
        let mut buf = [0u8; 16];
        let mut w = Writer::new(&mut buf);
        h.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let decoded = Bvlc6Header::decode(&mut r).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn bvlc6_wrong_type_byte_fails() {
        let mut r = Reader::new(&[0x81, 0x01, 0x00, 0x0A, 0x00, 0x00, 0x00]);
        assert!(Bvlc6Header::decode(&mut r).is_err());
    }

    #[test]
    fn bvlc6_unknown_function_decodes() {
        let mut r = Reader::new(&[BVLC_TYPE_BIP6, 0xEE, 0x00, 0x07, 0x00, 0x00, 0x00]);
        let decoded = Bvlc6Header::decode(&mut r).unwrap();
        assert_eq!(decoded.function, Bvlc6Function::Unknown(0xEE));
    }

    #[test]
    fn function_roundtrip_all_known() {
        let functions = [
            Bvlc6Function::Result,
            Bvlc6Function::OriginalUnicastNpdu,
            Bvlc6Function::OriginalBroadcastNpdu,
            Bvlc6Function::AddressResolution,
            Bvlc6Function::ForwardedAddressResolution,
            Bvlc6Function::AddressResolutionAck,
            Bvlc6Function::VirtualAddressResolution,
            Bvlc6Function::VirtualAddressResolutionAck,
            Bvlc6Function::ForwardedNpdu,
            Bvlc6Function::RegisterForeignDevice,
            Bvlc6Function::DeleteForeignDeviceTableEntry,
            Bvlc6Function::SecureBvll,
            Bvlc6Function::DistributeBroadcastToNetwork,
        ];
        for f in &functions {
            assert_eq!(Bvlc6Function::from_u8(f.to_u8()), *f);
        }
    }
}
