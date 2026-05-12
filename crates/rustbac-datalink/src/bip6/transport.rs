//! BACnet/IPv6 (Annex U) transport over UDP multicast.
//!
//! Implements [`DataLink`] for BACnet/IPv6 using IPv6 multicast.
//! The default multicast group is `FF05::BAC0` (site-local scope).

use crate::bip6::bvlc6::{Bvlc6Function, Bvlc6Header};
use crate::{DataLink, DataLinkAddress, DataLinkError};
use rustbac_core::encoding::{reader::Reader, writer::Writer};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6};
use std::sync::Arc;
use tokio::net::UdpSocket;

/// Default BACnet/IPv6 multicast group (site-local scope).
pub const DEFAULT_BIP6_MULTICAST: Ipv6Addr = Ipv6Addr::new(0xFF05, 0, 0, 0, 0, 0, 0, 0xBAC0);

/// Maximum BIP6 frame length.
const MAX_BIP6_FRAME_LEN: usize = 1500;

/// BACnet/IPv6 transport using UDP multicast.
#[derive(Debug, Clone)]
pub struct BacnetIp6Transport {
    socket: Arc<UdpSocket>,
    multicast_group: Ipv6Addr,
    port: u16,
}

impl BacnetIp6Transport {
    /// Bind to a local IPv6 address, joining the specified multicast group.
    ///
    /// # Arguments
    /// * `addr` — The local IPv6 address and port to bind to (e.g. `[::]:47808`).
    /// * `multicast` — The multicast group to join (default: `FF05::BAC0`).
    /// * `if_index` — The interface index for multicast group membership.
    pub async fn bind(
        addr: SocketAddrV6,
        multicast: Ipv6Addr,
        if_index: u32,
    ) -> Result<Self, DataLinkError> {
        let socket = UdpSocket::bind(addr).await?;

        // Join multicast group.
        let std_socket = socket.into_std()?;
        std_socket
            .join_multicast_v6(&multicast, if_index)
            .map_err(|e| {
                DataLinkError::Io(std::io::Error::other(format!(
                    "failed to join multicast group: {e}"
                )))
            })?;
        let socket = UdpSocket::from_std(std_socket)?;
        let port = addr.port();

        Ok(Self {
            socket: Arc::new(socket),
            multicast_group: multicast,
            port,
        })
    }

    /// The local address this transport is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr, DataLinkError> {
        self.socket.local_addr().map_err(DataLinkError::Io)
    }

    /// The multicast group this transport is using.
    pub fn multicast_group(&self) -> Ipv6Addr {
        self.multicast_group
    }
}

impl DataLink for BacnetIp6Transport {
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError> {
        let target = match address {
            DataLinkAddress::Ip(addr) => addr,
            DataLinkAddress::Mstp(_) => return Err(DataLinkError::InvalidFrame),
        };

        let is_broadcast = match target.ip() {
            IpAddr::V6(v6) => v6.is_multicast(),
            _ => false,
        };

        let function = if is_broadcast {
            Bvlc6Function::OriginalBroadcastNpdu
        } else {
            Bvlc6Function::OriginalUnicastNpdu
        };

        let mut frame = [0u8; MAX_BIP6_FRAME_LEN];
        let total_len = 7usize
            .checked_add(payload.len())
            .ok_or(DataLinkError::FrameTooLarge)?;
        if total_len > frame.len() {
            return Err(DataLinkError::FrameTooLarge);
        }

        let mut w = Writer::new(&mut frame);
        Bvlc6Header {
            function,
            length: total_len as u16,
            source_virtual_address: [0, 0, 0], // Local device, virtual address 0
        }
        .encode(&mut w)
        .map_err(|_| DataLinkError::InvalidFrame)?;
        w.write_all(payload)
            .map_err(|_| DataLinkError::FrameTooLarge)?;

        let send_target = if is_broadcast {
            SocketAddr::new(IpAddr::V6(self.multicast_group), self.port)
        } else {
            target
        };

        self.socket.send_to(w.as_written(), send_target).await?;
        Ok(())
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
        let mut frame = [0u8; MAX_BIP6_FRAME_LEN];
        let (n, src) = self.socket.recv_from(&mut frame).await?;
        let mut r = Reader::new(&frame[..n]);
        let hdr = Bvlc6Header::decode(&mut r).map_err(|_| DataLinkError::InvalidFrame)?;

        match hdr.function {
            Bvlc6Function::OriginalUnicastNpdu | Bvlc6Function::OriginalBroadcastNpdu => {
                let payload_len = hdr.length as usize - 7;
                let payload = r
                    .read_exact(payload_len)
                    .map_err(|_| DataLinkError::InvalidFrame)?;
                if payload.len() > buf.len() {
                    return Err(DataLinkError::FrameTooLarge);
                }
                buf[..payload.len()].copy_from_slice(payload);
                Ok((payload.len(), DataLinkAddress::Ip(src)))
            }
            Bvlc6Function::ForwardedNpdu => {
                // Forwarded NPDU includes original source address (18 bytes: 16 IPv6 + 2 port)
                let forwarded = r
                    .read_exact(hdr.length as usize - 7)
                    .map_err(|_| DataLinkError::InvalidFrame)?;
                if forwarded.len() < 18 {
                    return Err(DataLinkError::InvalidFrame);
                }
                let mut ipv6_bytes = [0u8; 16];
                ipv6_bytes.copy_from_slice(&forwarded[..16]);
                let origin_ip = Ipv6Addr::from(ipv6_bytes);
                let origin_port = u16::from_be_bytes([forwarded[16], forwarded[17]]);
                let payload = &forwarded[18..];
                if payload.len() > buf.len() {
                    return Err(DataLinkError::FrameTooLarge);
                }
                buf[..payload.len()].copy_from_slice(payload);
                Ok((
                    payload.len(),
                    DataLinkAddress::Ip(SocketAddr::new(IpAddr::V6(origin_ip), origin_port)),
                ))
            }
            Bvlc6Function::DistributeBroadcastToNetwork => {
                let payload_len = hdr.length as usize - 7;
                let payload = r
                    .read_exact(payload_len)
                    .map_err(|_| DataLinkError::InvalidFrame)?;
                if payload.len() > buf.len() {
                    return Err(DataLinkError::FrameTooLarge);
                }
                buf[..payload.len()].copy_from_slice(payload);
                Ok((payload.len(), DataLinkAddress::Ip(src)))
            }
            Bvlc6Function::Unknown(v) => Err(DataLinkError::UnsupportedBvlcFunction(v)),
            _ => Err(DataLinkError::InvalidFrame),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_multicast_group() {
        // FF05::BAC0 = site-local multicast for BACnet
        assert_eq!(
            DEFAULT_BIP6_MULTICAST,
            Ipv6Addr::new(0xFF05, 0, 0, 0, 0, 0, 0, 0xBAC0)
        );
    }
}
