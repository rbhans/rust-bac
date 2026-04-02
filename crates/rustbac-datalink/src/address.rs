use core::fmt;

#[cfg(feature = "std")]
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// A data-link-layer address identifying a BACnet peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataLinkAddress {
    /// A BACnet/IP endpoint (IPv4 or IPv6 socket address).
    #[cfg(feature = "std")]
    Ip(SocketAddr),
    /// An MS/TP node MAC address (0–127, or 255 for broadcast).
    Mstp(u8),
}

impl DataLinkAddress {
    /// The default BACnet/IP UDP port (`0xBAC0` = 47808).
    pub const BACNET_IP_DEFAULT_PORT: u16 = 47808;

    /// Returns a broadcast address on the given port.
    #[cfg(feature = "std")]
    pub fn local_broadcast(port: u16) -> Self {
        Self::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port))
    }

    #[cfg(feature = "std")]
    pub fn bacnet_default(addr: IpAddr) -> Self {
        Self::Ip(SocketAddr::new(addr, Self::BACNET_IP_DEFAULT_PORT))
    }

    /// Returns the inner [`SocketAddr`] if this is an `Ip` address.
    ///
    /// # Panics
    ///
    /// Panics if called on an `Mstp` address.
    #[cfg(feature = "std")]
    pub fn as_socket_addr(self) -> SocketAddr {
        match self {
            Self::Ip(addr) => addr,
            Self::Mstp(_) => panic!("as_socket_addr called on Mstp address"),
        }
    }

    /// Returns a broadcast address for IPv6 multicast on the given group and port.
    #[cfg(feature = "std")]
    pub fn ipv6_broadcast(group: std::net::Ipv6Addr, port: u16) -> Self {
        Self::Ip(SocketAddr::new(IpAddr::V6(group), port))
    }
}

impl fmt::Display for DataLinkAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "std")]
            Self::Ip(addr) => write!(f, "{addr}"),
            Self::Mstp(mac) => write!(f, "mstp:{mac}"),
        }
    }
}
