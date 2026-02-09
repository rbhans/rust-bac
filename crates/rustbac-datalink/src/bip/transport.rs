use crate::bip::bvlc::{BvlcFunction, BvlcHeader};
use crate::{DataLink, DataLinkAddress, DataLinkError};
use rustbac_core::encoding::{reader::Reader, writer::Writer};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, Instant};

const MAX_BIP_FRAME_LEN: usize = 1600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastDistributionEntry {
    pub address: SocketAddrV4,
    pub mask: Ipv4Addr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignDeviceTableEntry {
    pub address: SocketAddrV4,
    pub ttl_seconds: u16,
    pub remaining_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct BacnetIpTransport {
    socket: Arc<UdpSocket>,
    bbmd: Option<SocketAddr>,
    bbmd_command_lock: Arc<Mutex<()>>,
}

impl BacnetIpTransport {
    pub async fn bind(bind_addr: SocketAddr) -> Result<Self, DataLinkError> {
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket: Arc::new(socket),
            bbmd: None,
            bbmd_command_lock: Arc::new(Mutex::new(())),
        })
    }

    pub async fn bind_foreign(
        bind_addr: SocketAddr,
        bbmd_addr: SocketAddr,
    ) -> Result<Self, DataLinkError> {
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket: Arc::new(socket),
            bbmd: Some(bbmd_addr),
            bbmd_command_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, DataLinkError> {
        self.socket.local_addr().map_err(DataLinkError::Io)
    }

    pub fn bbmd_addr(&self) -> Option<SocketAddr> {
        self.bbmd
    }

    fn require_bbmd(&self) -> Result<SocketAddr, DataLinkError> {
        self.bbmd.ok_or(DataLinkError::BbmdNotConfigured)
    }

    fn parse_bvlc_result(payload: &[u8]) -> Result<(), DataLinkError> {
        if payload.len() < 2 {
            return Err(DataLinkError::InvalidFrame);
        }
        let code = u16::from_be_bytes([payload[0], payload[1]]);
        if code == 0 {
            Ok(())
        } else {
            Err(DataLinkError::BvlcResult(code))
        }
    }

    async fn send_bvlc_to_bbmd(
        &self,
        function: BvlcFunction,
        payload: &[u8],
    ) -> Result<(), DataLinkError> {
        let bbmd = self.bbmd.ok_or(DataLinkError::BbmdNotConfigured)?;
        let total_len = 4usize
            .checked_add(payload.len())
            .ok_or(DataLinkError::FrameTooLarge)?;
        if total_len > usize::from(u16::MAX) {
            return Err(DataLinkError::FrameTooLarge);
        }

        let mut frame = vec![0u8; total_len];
        let mut w = Writer::new(&mut frame);
        BvlcHeader {
            function,
            length: total_len as u16,
        }
        .encode(&mut w)
        .map_err(|_| DataLinkError::InvalidFrame)?;
        w.write_all(payload)
            .map_err(|_| DataLinkError::InvalidFrame)?;

        self.socket.send_to(w.as_written(), bbmd).await?;
        Ok(())
    }

    async fn recv_bvlc_reply(
        &self,
        expected: BvlcFunction,
        timeout_duration: Duration,
    ) -> Result<Vec<u8>, DataLinkError> {
        let bbmd = self.require_bbmd()?;
        let deadline = Instant::now() + timeout_duration;
        let mut rx = [0u8; 1600];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(DataLinkError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "bbmd response timeout",
                )));
            }

            let (n, src) = timeout(remaining, self.socket.recv_from(&mut rx))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "bbmd response timeout"))?
                .map_err(DataLinkError::Io)?;
            if src != bbmd {
                continue;
            }

            let mut r = Reader::new(&rx[..n]);
            let hdr = BvlcHeader::decode(&mut r).map_err(|_| DataLinkError::InvalidFrame)?;
            let payload = r
                .read_exact(hdr.length as usize - 4)
                .map_err(|_| DataLinkError::InvalidFrame)?;

            if hdr.function == expected {
                return Ok(payload.to_vec());
            }

            if hdr.function == BvlcFunction::Result {
                Self::parse_bvlc_result(payload)?;
                if expected == BvlcFunction::Result {
                    return Ok(payload.to_vec());
                }
                return Err(DataLinkError::InvalidFrame);
            }
        }
    }

    pub async fn register_foreign_device_no_wait(
        &self,
        ttl_seconds: u16,
    ) -> Result<(), DataLinkError> {
        let _guard = self.bbmd_command_lock.lock().await;
        let payload = ttl_seconds.to_be_bytes();
        self.send_bvlc_to_bbmd(BvlcFunction::RegisterForeignDevice, &payload)
            .await
    }

    pub async fn register_foreign_device(&self, ttl_seconds: u16) -> Result<(), DataLinkError> {
        let _guard = self.bbmd_command_lock.lock().await;
        let payload = ttl_seconds.to_be_bytes();
        self.send_bvlc_to_bbmd(BvlcFunction::RegisterForeignDevice, &payload)
            .await?;
        let payload = self
            .recv_bvlc_reply(BvlcFunction::Result, Duration::from_secs(2))
            .await?;
        Self::parse_bvlc_result(&payload)
    }

    pub async fn read_broadcast_distribution_table(
        &self,
    ) -> Result<Vec<BroadcastDistributionEntry>, DataLinkError> {
        let _guard = self.bbmd_command_lock.lock().await;
        self.send_bvlc_to_bbmd(BvlcFunction::ReadBroadcastDistributionTable, &[])
            .await?;
        let payload = self
            .recv_bvlc_reply(
                BvlcFunction::ReadBroadcastDistributionTableAck,
                Duration::from_secs(2),
            )
            .await?;
        if payload.len() % 10 != 0 {
            return Err(DataLinkError::InvalidFrame);
        }

        let mut out = Vec::with_capacity(payload.len() / 10);
        for chunk in payload.chunks_exact(10) {
            out.push(BroadcastDistributionEntry {
                address: SocketAddrV4::new(
                    Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]),
                    u16::from_be_bytes([chunk[4], chunk[5]]),
                ),
                mask: Ipv4Addr::new(chunk[6], chunk[7], chunk[8], chunk[9]),
            });
        }
        Ok(out)
    }

    pub async fn write_broadcast_distribution_table(
        &self,
        entries: &[BroadcastDistributionEntry],
    ) -> Result<(), DataLinkError> {
        let _guard = self.bbmd_command_lock.lock().await;
        let mut payload = Vec::with_capacity(entries.len() * 10);
        for entry in entries {
            payload.extend_from_slice(&entry.address.ip().octets());
            payload.extend_from_slice(&entry.address.port().to_be_bytes());
            payload.extend_from_slice(&entry.mask.octets());
        }

        self.send_bvlc_to_bbmd(BvlcFunction::WriteBroadcastDistributionTable, &payload)
            .await?;
        let payload = self
            .recv_bvlc_reply(BvlcFunction::Result, Duration::from_secs(2))
            .await?;
        Self::parse_bvlc_result(&payload)
    }

    pub async fn read_foreign_device_table(
        &self,
    ) -> Result<Vec<ForeignDeviceTableEntry>, DataLinkError> {
        let _guard = self.bbmd_command_lock.lock().await;
        self.send_bvlc_to_bbmd(BvlcFunction::ReadForeignDeviceTable, &[])
            .await?;
        let payload = self
            .recv_bvlc_reply(
                BvlcFunction::ReadForeignDeviceTableAck,
                Duration::from_secs(2),
            )
            .await?;
        if payload.len() % 10 != 0 {
            return Err(DataLinkError::InvalidFrame);
        }

        let mut out = Vec::with_capacity(payload.len() / 10);
        for chunk in payload.chunks_exact(10) {
            out.push(ForeignDeviceTableEntry {
                address: SocketAddrV4::new(
                    Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]),
                    u16::from_be_bytes([chunk[4], chunk[5]]),
                ),
                ttl_seconds: u16::from_be_bytes([chunk[6], chunk[7]]),
                remaining_seconds: u16::from_be_bytes([chunk[8], chunk[9]]),
            });
        }
        Ok(out)
    }

    pub async fn delete_foreign_device_table_entry(
        &self,
        address: SocketAddrV4,
    ) -> Result<(), DataLinkError> {
        let _guard = self.bbmd_command_lock.lock().await;
        let mut payload = [0u8; 6];
        payload[..4].copy_from_slice(&address.ip().octets());
        payload[4..].copy_from_slice(&address.port().to_be_bytes());
        self.send_bvlc_to_bbmd(BvlcFunction::DeleteForeignDeviceTableEntry, &payload)
            .await?;
        let payload = self
            .recv_bvlc_reply(BvlcFunction::Result, Duration::from_secs(2))
            .await?;
        Self::parse_bvlc_result(&payload)
    }
}

impl DataLink for BacnetIpTransport {
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError> {
        let addr = address.as_socket_addr();
        let is_broadcast = matches!(addr.ip(), IpAddr::V4(v4) if v4.is_broadcast());

        let (function, target_addr) = if is_broadcast {
            if let Some(bbmd) = self.bbmd {
                (BvlcFunction::DistributeBroadcastToNetwork, bbmd)
            } else {
                (BvlcFunction::OriginalBroadcastNpdu, addr)
            }
        } else {
            (BvlcFunction::OriginalUnicastNpdu, addr)
        };

        let mut frame = [0u8; MAX_BIP_FRAME_LEN];
        let total_len = 4usize
            .checked_add(payload.len())
            .ok_or(DataLinkError::FrameTooLarge)?;
        if total_len > frame.len() {
            return Err(DataLinkError::FrameTooLarge);
        }

        let mut w = Writer::new(&mut frame);
        BvlcHeader {
            function,
            length: total_len as u16,
        }
        .encode(&mut w)
        .map_err(|_| DataLinkError::InvalidFrame)?;
        w.write_all(payload)
            .map_err(|_| DataLinkError::FrameTooLarge)?;

        self.socket.send_to(w.as_written(), target_addr).await?;
        Ok(())
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
        let mut frame = [0u8; MAX_BIP_FRAME_LEN];
        let (n, src) = self.socket.recv_from(&mut frame).await?;
        let mut r = Reader::new(&frame[..n]);
        let hdr = BvlcHeader::decode(&mut r).map_err(|_| DataLinkError::InvalidFrame)?;

        match hdr.function {
            BvlcFunction::OriginalUnicastNpdu
            | BvlcFunction::OriginalBroadcastNpdu
            | BvlcFunction::DistributeBroadcastToNetwork => {
                let payload_len = hdr.length as usize - 4;
                let payload = r
                    .read_exact(payload_len)
                    .map_err(|_| DataLinkError::InvalidFrame)?;
                if payload.len() > buf.len() {
                    return Err(DataLinkError::FrameTooLarge);
                }
                buf[..payload.len()].copy_from_slice(payload);
                Ok((payload.len(), DataLinkAddress::Ip(src)))
            }
            BvlcFunction::ForwardedNpdu => {
                let forwarded = r
                    .read_exact(hdr.length as usize - 4)
                    .map_err(|_| DataLinkError::InvalidFrame)?;
                if forwarded.len() < 6 {
                    return Err(DataLinkError::InvalidFrame);
                }
                let origin_ip =
                    Ipv4Addr::new(forwarded[0], forwarded[1], forwarded[2], forwarded[3]);
                let origin_port = u16::from_be_bytes([forwarded[4], forwarded[5]]);
                let payload = &forwarded[6..];
                if payload.len() > buf.len() {
                    return Err(DataLinkError::FrameTooLarge);
                }
                buf[..payload.len()].copy_from_slice(payload);
                Ok((
                    payload.len(),
                    DataLinkAddress::Ip(SocketAddr::new(IpAddr::V4(origin_ip), origin_port)),
                ))
            }
            BvlcFunction::Unknown(v) => Err(DataLinkError::UnsupportedBvlcFunction(v)),
            _ => Err(DataLinkError::InvalidFrame),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BacnetIpTransport, BroadcastDistributionEntry, ForeignDeviceTableEntry};
    use crate::bip::bvlc::{BvlcFunction, BvlcHeader, BVLC_TYPE_BIP};
    use crate::{DataLink, DataLinkAddress, DataLinkError};
    use rustbac_core::encoding::{reader::Reader, writer::Writer};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
    use tokio::net::UdpSocket;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn recv_forwarded_npdu_returns_forwarded_origin() {
        let transport =
            BacnetIpTransport::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
                .await
                .unwrap();
        let target = transport.local_addr().unwrap();
        let sender = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();

        let mut frame = [0u8; 64];
        let mut w = Writer::new(&mut frame);
        BvlcHeader {
            function: BvlcFunction::ForwardedNpdu,
            length: 4 + 6 + 3,
        }
        .encode(&mut w)
        .unwrap();
        w.write_all(&[10, 1, 2, 3]).unwrap();
        w.write_be_u16(47808).unwrap();
        w.write_all(&[1, 2, 3]).unwrap();

        sender.send_to(w.as_written(), target).await.unwrap();

        let mut out = [0u8; 16];
        let (n, src) = transport.recv(&mut out).await.unwrap();
        assert_eq!(n, 3);
        assert_eq!(&out[..3], &[1, 2, 3]);
        assert_eq!(
            src,
            DataLinkAddress::Ip(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)),
                47808
            ))
        );
    }

    #[tokio::test]
    async fn register_foreign_device_success() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();

        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        let responder = tokio::spawn(async move {
            let mut recv = [0u8; 64];
            let (n, src) = bbmd.recv_from(&mut recv).await.unwrap();
            let mut r = Reader::new(&recv[..n]);
            let hdr = BvlcHeader::decode(&mut r).unwrap();
            assert_eq!(hdr.function, BvlcFunction::RegisterForeignDevice);

            let reply = [BVLC_TYPE_BIP, 0x00, 0x00, 0x06, 0x00, 0x00];
            bbmd.send_to(&reply, src).await.unwrap();
        });

        transport.register_foreign_device(60).await.unwrap();
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn register_foreign_device_no_wait_sends_ttl() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();
        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        transport.register_foreign_device_no_wait(90).await.unwrap();

        let mut recv = [0u8; 64];
        let (n, _) = bbmd.recv_from(&mut recv).await.unwrap();
        let mut r = Reader::new(&recv[..n]);
        let hdr = BvlcHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.function, BvlcFunction::RegisterForeignDevice);
        assert_eq!(r.read_be_u16().unwrap(), 90);
    }

    #[tokio::test]
    async fn read_broadcast_distribution_table_parses_entries() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();
        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        let responder = tokio::spawn(async move {
            let mut recv = [0u8; 128];
            let (n, src) = bbmd.recv_from(&mut recv).await.unwrap();
            let mut r = Reader::new(&recv[..n]);
            let hdr = BvlcHeader::decode(&mut r).unwrap();
            assert_eq!(hdr.function, BvlcFunction::ReadBroadcastDistributionTable);

            let mut reply = [0u8; 32];
            let mut w = Writer::new(&mut reply);
            BvlcHeader {
                function: BvlcFunction::ReadBroadcastDistributionTableAck,
                length: 14,
            }
            .encode(&mut w)
            .unwrap();
            w.write_all(&[192, 168, 10, 20]).unwrap();
            w.write_be_u16(47808).unwrap();
            w.write_all(&[255, 255, 255, 0]).unwrap();
            bbmd.send_to(w.as_written(), src).await.unwrap();
        });

        let entries = transport.read_broadcast_distribution_table().await.unwrap();
        assert_eq!(
            entries,
            vec![BroadcastDistributionEntry {
                address: SocketAddrV4::new(Ipv4Addr::new(192, 168, 10, 20), 47808),
                mask: Ipv4Addr::new(255, 255, 255, 0),
            }]
        );
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn write_broadcast_distribution_table_sends_entries() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();
        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        let responder = tokio::spawn(async move {
            let mut recv = [0u8; 128];
            let (n, src) = bbmd.recv_from(&mut recv).await.unwrap();
            let mut r = Reader::new(&recv[..n]);
            let hdr = BvlcHeader::decode(&mut r).unwrap();
            assert_eq!(hdr.function, BvlcFunction::WriteBroadcastDistributionTable);
            let payload = r.read_exact(hdr.length as usize - 4).unwrap();
            assert_eq!(payload, &[10, 1, 2, 3, 0xBA, 0xC0, 255, 255, 255, 0][..]);

            let reply = [BVLC_TYPE_BIP, 0x00, 0x00, 0x06, 0x00, 0x00];
            bbmd.send_to(&reply, src).await.unwrap();
        });

        let entries = [BroadcastDistributionEntry {
            address: SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 47808),
            mask: Ipv4Addr::new(255, 255, 255, 0),
        }];
        transport
            .write_broadcast_distribution_table(&entries)
            .await
            .unwrap();
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn read_foreign_device_table_parses_entries() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();
        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        let responder = tokio::spawn(async move {
            let mut recv = [0u8; 128];
            let (n, src) = bbmd.recv_from(&mut recv).await.unwrap();
            let mut r = Reader::new(&recv[..n]);
            let hdr = BvlcHeader::decode(&mut r).unwrap();
            assert_eq!(hdr.function, BvlcFunction::ReadForeignDeviceTable);

            let mut reply = [0u8; 32];
            let mut w = Writer::new(&mut reply);
            BvlcHeader {
                function: BvlcFunction::ReadForeignDeviceTableAck,
                length: 14,
            }
            .encode(&mut w)
            .unwrap();
            w.write_all(&[172, 16, 0, 42]).unwrap();
            w.write_be_u16(47808).unwrap();
            w.write_be_u16(120).unwrap();
            w.write_be_u16(90).unwrap();
            bbmd.send_to(w.as_written(), src).await.unwrap();
        });

        let entries = transport.read_foreign_device_table().await.unwrap();
        assert_eq!(
            entries,
            vec![ForeignDeviceTableEntry {
                address: SocketAddrV4::new(Ipv4Addr::new(172, 16, 0, 42), 47808),
                ttl_seconds: 120,
                remaining_seconds: 90,
            }]
        );
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn delete_foreign_device_table_entry_sends_target() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();
        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        let responder = tokio::spawn(async move {
            let mut recv = [0u8; 128];
            let (n, src) = bbmd.recv_from(&mut recv).await.unwrap();
            let mut r = Reader::new(&recv[..n]);
            let hdr = BvlcHeader::decode(&mut r).unwrap();
            assert_eq!(hdr.function, BvlcFunction::DeleteForeignDeviceTableEntry);
            let payload = r.read_exact(hdr.length as usize - 4).unwrap();
            assert_eq!(payload, &[10, 20, 30, 40, 0xBA, 0xC0][..]);

            let reply = [BVLC_TYPE_BIP, 0x00, 0x00, 0x06, 0x00, 0x00];
            bbmd.send_to(&reply, src).await.unwrap();
        });

        transport
            .delete_foreign_device_table_entry(SocketAddrV4::new(
                Ipv4Addr::new(10, 20, 30, 40),
                47808,
            ))
            .await
            .unwrap();
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn broadcast_uses_distribute_to_network_when_bbmd_configured() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();

        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        transport
            .send(DataLinkAddress::local_broadcast(47808), &[1, 2, 3])
            .await
            .unwrap();

        let mut recv = [0u8; 64];
        let (n, _) = bbmd.recv_from(&mut recv).await.unwrap();
        let mut r = Reader::new(&recv[..n]);
        let hdr = BvlcHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.function, BvlcFunction::DistributeBroadcastToNetwork);
    }

    #[tokio::test]
    async fn bbmd_admin_commands_are_serialized() {
        let bbmd = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let bbmd_addr = bbmd.local_addr().unwrap();
        let transport = BacnetIpTransport::bind_foreign(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            bbmd_addr,
        )
        .await
        .unwrap();

        let t1 = transport.clone();
        let t2 = transport.clone();

        let first = tokio::spawn(async move { t1.read_broadcast_distribution_table().await });
        let second = tokio::spawn(async move { t2.read_foreign_device_table().await });

        let mut recv = [0u8; 128];
        let (n1, src1) = bbmd.recv_from(&mut recv).await.unwrap();
        let mut r1 = Reader::new(&recv[..n1]);
        let hdr1 = BvlcHeader::decode(&mut r1).unwrap();
        assert_eq!(hdr1.function, BvlcFunction::ReadBroadcastDistributionTable);

        // Second command should not send until first receives a response.
        let no_second_yet = timeout(Duration::from_millis(100), bbmd.recv_from(&mut recv)).await;
        assert!(no_second_yet.is_err());

        let mut reply1 = [0u8; 14];
        let mut w1 = Writer::new(&mut reply1);
        BvlcHeader {
            function: BvlcFunction::ReadBroadcastDistributionTableAck,
            length: 14,
        }
        .encode(&mut w1)
        .unwrap();
        w1.write_all(&[192, 168, 1, 1]).unwrap();
        w1.write_be_u16(47808).unwrap();
        w1.write_all(&[255, 255, 255, 0]).unwrap();
        bbmd.send_to(w1.as_written(), src1).await.unwrap();

        let (n2, src2) = bbmd.recv_from(&mut recv).await.unwrap();
        let mut r2 = Reader::new(&recv[..n2]);
        let hdr2 = BvlcHeader::decode(&mut r2).unwrap();
        assert_eq!(hdr2.function, BvlcFunction::ReadForeignDeviceTable);

        let mut reply2 = [0u8; 14];
        let mut w2 = Writer::new(&mut reply2);
        BvlcHeader {
            function: BvlcFunction::ReadForeignDeviceTableAck,
            length: 14,
        }
        .encode(&mut w2)
        .unwrap();
        w2.write_all(&[10, 0, 0, 2]).unwrap();
        w2.write_be_u16(47808).unwrap();
        w2.write_be_u16(60).unwrap();
        w2.write_be_u16(30).unwrap();
        bbmd.send_to(w2.as_written(), src2).await.unwrap();

        let first_entries = first.await.unwrap().unwrap();
        let second_entries = second.await.unwrap().unwrap();
        assert_eq!(first_entries.len(), 1);
        assert_eq!(second_entries.len(), 1);
    }

    #[tokio::test]
    async fn unknown_bvlc_function_errors() {
        let transport =
            BacnetIpTransport::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
                .await
                .unwrap();
        let target = transport.local_addr().unwrap();
        let sender = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();

        let frame = [BVLC_TYPE_BIP, 0x99, 0x00, 0x04];
        sender.send_to(&frame, target).await.unwrap();

        let mut out = [0u8; 16];
        let err = transport.recv(&mut out).await.unwrap_err();
        assert!(matches!(err, DataLinkError::UnsupportedBvlcFunction(0x99)));
    }
}
