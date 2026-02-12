//! PCAP packet capture via a [`DataLink`](crate::DataLink) wrapper.
//!
//! [`CapturingDataLink`] wraps any transport and writes all sent/received
//! frames to a PCAP file for offline analysis (e.g. with Wireshark).

use crate::{DataLink, DataLinkAddress, DataLinkError};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// PCAP link type for raw BACnet/IP (UDP payload).
///
/// Using `USER0` (147) since there is no official link type for BACnet
/// application-layer capture.
const PCAP_LINK_TYPE_USER0: u32 = 147;
const PCAP_MAGIC: u32 = 0xa1b2c3d4;
const PCAP_VERSION_MAJOR: u16 = 2;
const PCAP_VERSION_MINOR: u16 = 4;
const PCAP_MAX_SNAPLEN: u32 = 65535;

/// Direction of a captured packet.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    In,
    Out,
}

/// A PCAP writer that writes the global header once and appends packet records.
struct PcapWriter<W: Write + Send> {
    inner: W,
}

impl<W: Write + Send> PcapWriter<W> {
    fn new(mut writer: W) -> io::Result<Self> {
        // Write PCAP global header.
        writer.write_all(&PCAP_MAGIC.to_le_bytes())?;
        writer.write_all(&PCAP_VERSION_MAJOR.to_le_bytes())?;
        writer.write_all(&PCAP_VERSION_MINOR.to_le_bytes())?;
        writer.write_all(&0i32.to_le_bytes())?; // thiszone
        writer.write_all(&0u32.to_le_bytes())?; // sigfigs
        writer.write_all(&PCAP_MAX_SNAPLEN.to_le_bytes())?;
        writer.write_all(&PCAP_LINK_TYPE_USER0.to_le_bytes())?;
        writer.flush()?;
        Ok(Self { inner: writer })
    }

    fn write_packet(&mut self, data: &[u8]) -> io::Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let ts_sec = now.as_secs() as u32;
        let ts_usec = now.subsec_micros();
        let len = data.len() as u32;

        self.inner.write_all(&ts_sec.to_le_bytes())?;
        self.inner.write_all(&ts_usec.to_le_bytes())?;
        self.inner.write_all(&len.to_le_bytes())?; // incl_len
        self.inner.write_all(&len.to_le_bytes())?; // orig_len
        self.inner.write_all(data)?;
        self.inner.flush()
    }
}

/// A [`DataLink`] wrapper that captures all frames to a PCAP file.
///
/// ```no_run
/// # use rustbac_datalink::capture::CapturingDataLink;
/// # use rustbac_datalink::bip::transport::BacnetIpTransport;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let transport = BacnetIpTransport::bind("0.0.0.0:47808".parse()?).await?;
/// let capturing = CapturingDataLink::to_file(transport, "capture.pcap")?;
/// // Use `capturing` as your DataLink — all traffic is logged.
/// # Ok(())
/// # }
/// ```
pub struct CapturingDataLink<D: DataLink> {
    inner: D,
    writer: Arc<Mutex<PcapWriter<std::io::BufWriter<std::fs::File>>>>,
}

impl<D: DataLink> CapturingDataLink<D> {
    /// Create a new capturing wrapper that writes frames to the given file path.
    pub fn to_file(inner: D, path: impl AsRef<std::path::Path>) -> io::Result<Self> {
        let file = std::fs::File::create(path)?;
        let buf_writer = std::io::BufWriter::new(file);
        let pcap = PcapWriter::new(buf_writer)?;
        Ok(Self {
            inner,
            writer: Arc::new(Mutex::new(pcap)),
        })
    }
}

impl<D: DataLink> DataLink for CapturingDataLink<D> {
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError> {
        {
            let mut w = self.writer.lock().await;
            let _ = w.write_packet(payload);
        }
        self.inner.send(address, payload).await
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
        let result = self.inner.recv(buf).await?;
        {
            let mut w = self.writer.lock().await;
            let _ = w.write_packet(&buf[..result.0]);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcap_global_header_format() {
        let mut buf = Vec::new();
        let _writer = PcapWriter::new(&mut buf).unwrap();
        assert_eq!(buf.len(), 24); // PCAP global header is 24 bytes
        assert_eq!(
            u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            PCAP_MAGIC
        );
    }

    #[test]
    fn pcap_write_packet() {
        let mut buf = Vec::new();
        let mut writer = PcapWriter::new(&mut buf).unwrap();
        writer.write_packet(&[0x01, 0x02, 0x03]).unwrap();
        // 24 (header) + 16 (packet header) + 3 (data) = 43
        assert_eq!(buf.len(), 43);
    }
}
