use crate::encoding::{reader::Reader, writer::Writer};
use crate::{DecodeError, EncodeError};

/// BACnet network layer protocol version (always `0x01`).
pub const NPDU_VERSION: u8 = 0x01;

/// A network-layer address consisting of a network number and a MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpduAddress {
    /// The DNET/SNET network number.
    pub network: u16,
    /// MAC address bytes (up to 6).
    pub mac: [u8; 6],
    /// Number of valid bytes in `mac`.
    pub mac_len: u8,
}

/// BACnet Network Protocol Data Unit (NPDU) header.
///
/// Handles encoding and decoding of the NPDU including optional source/
/// destination addresses, hop count, and network-layer message fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Npdu {
    pub control: u8,
    pub destination: Option<NpduAddress>,
    pub source: Option<NpduAddress>,
    pub hop_count: Option<u8>,
    pub message_type: Option<u8>,
    pub vendor_id: Option<u16>,
}

impl Npdu {
    pub const fn new(control: u8) -> Self {
        Self {
            control,
            destination: None,
            source: None,
            hop_count: None,
            message_type: None,
            vendor_id: None,
        }
    }

    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.write_u8(NPDU_VERSION)?;
        w.write_u8(self.control)?;

        if let Some(dest) = self.destination {
            encode_addr(w, dest)?;
        }
        if let Some(src) = self.source {
            encode_addr(w, src)?;
        }
        if self.destination.is_some() {
            w.write_u8(self.hop_count.unwrap_or(255))?;
        }
        if (self.control & 0x80) != 0 {
            w.write_u8(self.message_type.unwrap_or(0))?;
            if matches!(self.message_type, Some(0x80..=0xFF)) {
                w.write_be_u16(self.vendor_id.unwrap_or(0))?;
            }
        }
        Ok(())
    }

    pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let version = r.read_u8()?;
        if version != NPDU_VERSION {
            return Err(DecodeError::InvalidValue);
        }

        let control = r.read_u8()?;
        let has_dest = (control & 0x20) != 0;
        let has_src = (control & 0x08) != 0;
        let is_network_msg = (control & 0x80) != 0;

        let destination = if has_dest {
            Some(decode_addr(r)?)
        } else {
            None
        };
        let source = if has_src { Some(decode_addr(r)?) } else { None };
        let hop_count = if has_dest { Some(r.read_u8()?) } else { None };

        let (message_type, vendor_id) = if is_network_msg {
            let mt = r.read_u8()?;
            let vid = if mt >= 0x80 {
                Some(r.read_be_u16()?)
            } else {
                None
            };
            (Some(mt), vid)
        } else {
            (None, None)
        };

        Ok(Self {
            control,
            destination,
            source,
            hop_count,
            message_type,
            vendor_id,
        })
    }
}

fn encode_addr(w: &mut Writer<'_>, addr: NpduAddress) -> Result<(), EncodeError> {
    if addr.mac_len as usize > addr.mac.len() {
        return Err(EncodeError::InvalidLength);
    }
    w.write_be_u16(addr.network)?;
    w.write_u8(addr.mac_len)?;
    w.write_all(&addr.mac[..addr.mac_len as usize])
}

fn decode_addr(r: &mut Reader<'_>) -> Result<NpduAddress, DecodeError> {
    let network = r.read_be_u16()?;
    let mac_len = r.read_u8()?;
    if mac_len as usize > 6 {
        return Err(DecodeError::InvalidLength);
    }
    let mut mac = [0u8; 6];
    let src = r.read_exact(mac_len as usize)?;
    mac[..mac_len as usize].copy_from_slice(src);
    Ok(NpduAddress {
        network,
        mac,
        mac_len,
    })
}

#[cfg(test)]
mod tests {
    use super::{Npdu, NpduAddress};
    use crate::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn npdu_roundtrip() {
        let mut p = Npdu::new(0x20);
        p.destination = Some(NpduAddress {
            network: 1,
            mac: [192, 168, 1, 2, 0xBA, 0xC0],
            mac_len: 6,
        });
        p.hop_count = Some(255);

        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        p.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let dec = Npdu::decode(&mut r).unwrap();
        assert_eq!(dec.control, p.control);
        assert_eq!(dec.destination.unwrap().network, 1);
    }

    #[test]
    fn network_message_vendor_id_only_for_vendor_types() {
        let mut p = Npdu::new(0x80);
        p.message_type = Some(0x80);
        p.vendor_id = Some(260);

        let mut buf = [0u8; 16];
        let mut w = Writer::new(&mut buf);
        p.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let dec = Npdu::decode(&mut r).unwrap();
        assert_eq!(dec.message_type, Some(0x80));
        assert_eq!(dec.vendor_id, Some(260));
    }
}
