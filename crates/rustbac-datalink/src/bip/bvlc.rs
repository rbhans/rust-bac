use rustbac_core::encoding::{reader::Reader, writer::Writer};
use rustbac_core::{DecodeError, EncodeError};

pub const BVLC_TYPE_BIP: u8 = 0x81;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BvlcFunction {
    Result,
    WriteBroadcastDistributionTable,
    ReadBroadcastDistributionTable,
    ReadBroadcastDistributionTableAck,
    ForwardedNpdu,
    RegisterForeignDevice,
    ReadForeignDeviceTable,
    ReadForeignDeviceTableAck,
    DeleteForeignDeviceTableEntry,
    DistributeBroadcastToNetwork,
    OriginalUnicastNpdu,
    OriginalBroadcastNpdu,
    Unknown(u8),
}

impl BvlcFunction {
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0x00 => Self::Result,
            0x01 => Self::WriteBroadcastDistributionTable,
            0x02 => Self::ReadBroadcastDistributionTable,
            0x03 => Self::ReadBroadcastDistributionTableAck,
            0x04 => Self::ForwardedNpdu,
            0x05 => Self::RegisterForeignDevice,
            0x06 => Self::ReadForeignDeviceTable,
            0x07 => Self::ReadForeignDeviceTableAck,
            0x08 => Self::DeleteForeignDeviceTableEntry,
            0x09 => Self::DistributeBroadcastToNetwork,
            0x0A => Self::OriginalUnicastNpdu,
            0x0B => Self::OriginalBroadcastNpdu,
            v => Self::Unknown(v),
        }
    }

    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Result => 0x00,
            Self::WriteBroadcastDistributionTable => 0x01,
            Self::ReadBroadcastDistributionTable => 0x02,
            Self::ReadBroadcastDistributionTableAck => 0x03,
            Self::ForwardedNpdu => 0x04,
            Self::RegisterForeignDevice => 0x05,
            Self::ReadForeignDeviceTable => 0x06,
            Self::ReadForeignDeviceTableAck => 0x07,
            Self::DeleteForeignDeviceTableEntry => 0x08,
            Self::DistributeBroadcastToNetwork => 0x09,
            Self::OriginalUnicastNpdu => 0x0A,
            Self::OriginalBroadcastNpdu => 0x0B,
            Self::Unknown(v) => v,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BvlcHeader {
    pub function: BvlcFunction,
    pub length: u16,
}

impl BvlcHeader {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.write_u8(BVLC_TYPE_BIP)?;
        w.write_u8(self.function.to_u8())?;
        w.write_be_u16(self.length)
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        if r.read_u8()? != BVLC_TYPE_BIP {
            return Err(DecodeError::InvalidValue);
        }
        let function = BvlcFunction::from_u8(r.read_u8()?);
        let length = r.read_be_u16()?;
        if length < 4 {
            return Err(DecodeError::InvalidLength);
        }
        Ok(Self { function, length })
    }
}

#[cfg(test)]
mod tests {
    use super::{BvlcFunction, BvlcHeader, BVLC_TYPE_BIP};
    use rustbac_core::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn bvlc_roundtrip() {
        let h = BvlcHeader {
            function: BvlcFunction::OriginalBroadcastNpdu,
            length: 12,
        };
        let mut buf = [0u8; 8];
        let mut w = Writer::new(&mut buf);
        h.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let decoded = BvlcHeader::decode(&mut r).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn bvlc_register_foreign_roundtrip() {
        let h = BvlcHeader {
            function: BvlcFunction::RegisterForeignDevice,
            length: 6,
        };
        let mut buf = [0u8; 8];
        let mut w = Writer::new(&mut buf);
        h.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let decoded = BvlcHeader::decode(&mut r).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn bvlc_read_tables_ack_roundtrip() {
        let h = BvlcHeader {
            function: BvlcFunction::ReadBroadcastDistributionTableAck,
            length: 14,
        };
        let mut buf = [0u8; 8];
        let mut w = Writer::new(&mut buf);
        h.encode(&mut w).unwrap();
        let mut r = Reader::new(w.as_written());
        let decoded = BvlcHeader::decode(&mut r).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn unknown_function_decodes() {
        let mut r = Reader::new(&[BVLC_TYPE_BIP, 0x99, 0, 4]);
        let decoded = BvlcHeader::decode(&mut r).unwrap();
        assert_eq!(decoded.function, BvlcFunction::Unknown(0x99));
    }
}
