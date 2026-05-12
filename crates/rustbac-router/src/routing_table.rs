//! BACnet routing table with direct and learned routes.

use rustbac_datalink::DataLinkAddress;
use std::collections::HashMap;

/// A routing table that maps BACnet network numbers to ports.
///
/// Routes are either **direct** (a port is directly connected to a network)
/// or **learned** (discovered via I-Am-Router-To-Network messages from
/// another router reachable through a port).
#[derive(Debug)]
pub struct RoutingTable {
    /// Directly connected networks: network → port index.
    direct: HashMap<u16, usize>,
    /// Learned routes: network → (port index, next-hop router address).
    learned: HashMap<u16, (usize, DataLinkAddress)>,
}

/// The result of a route lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteEntry {
    /// The network is directly connected to the port at the given index.
    Direct(usize),
    /// The network is reachable via the port at the given index through
    /// the router at the given address.
    Learned(usize, DataLinkAddress),
}

impl RoutingTable {
    /// Create an empty routing table.
    pub fn new() -> Self {
        Self {
            direct: HashMap::new(),
            learned: HashMap::new(),
        }
    }

    /// Register a directly connected network on the given port.
    pub fn add_direct_route(&mut self, network: u16, port_index: usize) {
        self.direct.insert(network, port_index);
        // Direct routes supersede learned routes.
        self.learned.remove(&network);
    }

    /// Record a learned route from an I-Am-Router-To-Network message.
    ///
    /// Learned routes do not override direct routes.
    pub fn learn_route(&mut self, network: u16, port_index: usize, via: DataLinkAddress) {
        if self.direct.contains_key(&network) {
            return; // Direct routes take precedence.
        }
        self.learned.insert(network, (port_index, via));
    }

    /// Look up the route for a destination network.
    pub fn lookup(&self, network: u16) -> Option<RouteEntry> {
        if let Some(&port) = self.direct.get(&network) {
            return Some(RouteEntry::Direct(port));
        }
        if let Some(&(port, addr)) = self.learned.get(&network) {
            return Some(RouteEntry::Learned(port, addr));
        }
        None
    }

    /// Return all directly connected network numbers.
    pub fn directly_connected_networks(&self) -> Vec<u16> {
        self.direct.keys().copied().collect()
    }

    /// Return all reachable network numbers (direct + learned).
    pub fn all_reachable_networks(&self) -> Vec<u16> {
        let mut nets: Vec<u16> = self
            .direct
            .keys()
            .chain(self.learned.keys())
            .copied()
            .collect();
        nets.sort_unstable();
        nets.dedup();
        nets
    }

    /// Remove all learned routes received through a specific port.
    pub fn remove_learned_via_port(&mut self, port_index: usize) {
        self.learned.retain(|_, (p, _)| *p != port_index);
    }
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> DataLinkAddress {
        DataLinkAddress::Ip(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1)),
            port,
        ))
    }

    #[test]
    fn direct_route_lookup() {
        let mut rt = RoutingTable::new();
        rt.add_direct_route(1, 0);
        rt.add_direct_route(2, 1);
        assert_eq!(rt.lookup(1), Some(RouteEntry::Direct(0)));
        assert_eq!(rt.lookup(2), Some(RouteEntry::Direct(1)));
        assert_eq!(rt.lookup(3), None);
    }

    #[test]
    fn learned_route_lookup() {
        let mut rt = RoutingTable::new();
        rt.learn_route(10, 0, addr(47808));
        assert_eq!(rt.lookup(10), Some(RouteEntry::Learned(0, addr(47808))));
    }

    #[test]
    fn direct_overrides_learned() {
        let mut rt = RoutingTable::new();
        rt.learn_route(5, 0, addr(47808));
        assert!(matches!(rt.lookup(5), Some(RouteEntry::Learned(_, _))));
        rt.add_direct_route(5, 1);
        assert_eq!(rt.lookup(5), Some(RouteEntry::Direct(1)));
    }

    #[test]
    fn learned_does_not_override_direct() {
        let mut rt = RoutingTable::new();
        rt.add_direct_route(5, 0);
        rt.learn_route(5, 1, addr(47808));
        assert_eq!(rt.lookup(5), Some(RouteEntry::Direct(0)));
    }

    #[test]
    fn all_reachable() {
        let mut rt = RoutingTable::new();
        rt.add_direct_route(1, 0);
        rt.learn_route(5, 0, addr(47808));
        let nets = rt.all_reachable_networks();
        assert_eq!(nets, vec![1, 5]);
    }

    #[test]
    fn remove_learned_via_port() {
        let mut rt = RoutingTable::new();
        rt.learn_route(10, 0, addr(47808));
        rt.learn_route(20, 1, addr(47809));
        rt.remove_learned_via_port(0);
        assert_eq!(rt.lookup(10), None);
        assert!(rt.lookup(20).is_some());
    }
}
