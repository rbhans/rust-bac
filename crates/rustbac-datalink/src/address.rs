use core::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataLinkAddress {
    Ip(SocketAddr),
}

impl DataLinkAddress {
    pub const BACNET_IP_DEFAULT_PORT: u16 = 47808;

    pub fn local_broadcast(port: u16) -> Self {
        Self::Ip(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port))
    }

    pub fn bacnet_default(addr: IpAddr) -> Self {
        Self::Ip(SocketAddr::new(addr, Self::BACNET_IP_DEFAULT_PORT))
    }

    pub fn as_socket_addr(self) -> SocketAddr {
        match self {
            Self::Ip(addr) => addr,
        }
    }
}

impl fmt::Display for DataLinkAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ip(addr) => write!(f, "{addr}"),
        }
    }
}
