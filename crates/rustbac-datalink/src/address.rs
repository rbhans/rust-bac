use core::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// A data-link-layer address identifying a BACnet peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataLinkAddress {
    /// A BACnet/IP endpoint (IPv4 or IPv6 socket address).
    Ip(SocketAddr),
    /// An MS/TP node MAC address (0–127, or 255 for broadcast).
    Mstp(u8),
}

impl DataLinkAddress {
    /// The default BACnet/IP UDP port (`0xBAC0` = 47808).
    pub const BACNET_IP_DEFAULT_PORT: u16 = 47808;

    /// Returns a broadcast address on the given port.
    pub fn local_broadcast(port: u16) -> Self {
        Self::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port))
    }

    pub fn bacnet_default(addr: IpAddr) -> Self {
        Self::Ip(SocketAddr::new(addr, Self::BACNET_IP_DEFAULT_PORT))
    }

    /// Returns the inner [`SocketAddr`] if this is an `Ip` address.
    ///
    /// # Panics
    ///
    /// Panics if called on an `Mstp` address.
    pub fn as_socket_addr(self) -> SocketAddr {
        match self {
            Self::Ip(addr) => addr,
            Self::Mstp(_) => panic!("as_socket_addr called on Mstp address"),
        }
    }
}

impl fmt::Display for DataLinkAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ip(addr) => write!(f, "{addr}"),
            Self::Mstp(mac) => write!(f, "mstp:{mac}"),
        }
    }
}
