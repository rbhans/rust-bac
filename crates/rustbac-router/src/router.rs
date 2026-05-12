//! BACnet multi-port network router.
//!
//! Forwards NPDUs between directly connected BACnet networks according
//! to ASHRAE 135 Clause 6, handling network-layer messages (Who-Is-Router,
//! I-Am-Router, etc.) and decrementing hop counts.

use crate::network_msg;
use crate::routing_table::{RouteEntry, RoutingTable};
use rustbac_core::encoding::{reader::Reader, writer::Writer};
use rustbac_core::npdu::{Npdu, NpduAddress};
use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Boxed future returned by [`RouterDataLink::send_frame`].
pub type SendFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DataLinkError>> + Send + 'a>>;

/// Boxed future returned by [`RouterDataLink::recv_frame`].
pub type RecvFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<(usize, DataLinkAddress), DataLinkError>>
            + Send
            + 'a,
    >,
>;

/// Object-safe data-link abstraction for the router.
///
/// The upstream `DataLink` trait uses native async methods and is not
/// object-safe.  This trait wraps the send/recv calls behind
/// [`std::pin::Pin`]`<Box<dyn Future>>` so we can store heterogeneous
/// ports in a single `Vec`.
pub trait RouterDataLink: Send + Sync {
    /// Send a frame to the given address.
    fn send_frame<'a>(&'a self, address: DataLinkAddress, payload: &'a [u8]) -> SendFuture<'a>;

    /// Receive a frame into `buf`, returning `(bytes_read, source_address)`.
    fn recv_frame<'a>(&'a self, buf: &'a mut [u8]) -> RecvFuture<'a>;
}

/// Implement `RouterDataLink` for `BacnetIpTransport` which has `Send` futures.
impl RouterDataLink for rustbac_datalink::BacnetIpTransport {
    fn send_frame<'a>(&'a self, address: DataLinkAddress, payload: &'a [u8]) -> SendFuture<'a> {
        Box::pin(self.send(address, payload))
    }

    fn recv_frame<'a>(&'a self, buf: &'a mut [u8]) -> RecvFuture<'a> {
        Box::pin(self.recv(buf))
    }
}

/// A port on the router, binding a network number to a data-link transport.
pub struct RouterPort {
    /// The BACnet network number assigned to this port.
    pub network: u16,
    /// The data-link transport for this port.
    pub datalink: Arc<dyn RouterDataLink>,
}

/// A multi-port BACnet network router.
///
/// Listens on all ports simultaneously and forwards frames based on DNET
/// lookup in the routing table. Handles network-layer messages for route
/// discovery and maintenance.
pub struct BacnetRouter {
    ports: Vec<RouterPort>,
    table: Arc<RwLock<RoutingTable>>,
}

impl BacnetRouter {
    /// Create a new router with the given ports.
    ///
    /// Each port's network number is automatically registered as a direct route.
    pub fn new(ports: Vec<RouterPort>) -> Self {
        let mut table = RoutingTable::new();
        for (i, port) in ports.iter().enumerate() {
            table.add_direct_route(port.network, i);
        }
        Self {
            ports,
            table: Arc::new(RwLock::new(table)),
        }
    }

    /// Run the router, spawning a receive task for each port.
    ///
    /// This method runs forever, forwarding frames between ports.
    pub async fn run(&self) {
        let mut handles = Vec::new();

        for (port_index, port) in self.ports.iter().enumerate() {
            let datalink = port.datalink.clone();
            let ingress_network = port.network;
            let table = self.table.clone();
            let ports: Vec<(u16, Arc<dyn RouterDataLink>)> = self
                .ports
                .iter()
                .map(|p| (p.network, p.datalink.clone()))
                .collect();

            let handle = tokio::spawn(async move {
                let mut buf = [0u8; 1500];
                loop {
                    match datalink.recv_frame(&mut buf).await {
                        Ok((n, source)) => {
                            if let Err(e) = handle_frame(
                                &buf[..n],
                                source,
                                port_index,
                                ingress_network,
                                &table,
                                &ports,
                            )
                            .await
                            {
                                log::debug!(
                                    "router port {port_index} (net {ingress_network}): {e:?}"
                                );
                            }
                        }
                        Err(e) => {
                            log::debug!(
                                "router port {port_index} (net {ingress_network}): recv error: {e:?}"
                            );
                            tokio::task::yield_now().await;
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks (they run forever).
        for h in handles {
            let _ = h.await;
        }
    }

    /// Get a read lock on the routing table (for inspection/testing).
    pub async fn routing_table(&self) -> tokio::sync::RwLockReadGuard<'_, RoutingTable> {
        self.table.read().await
    }
}

async fn handle_frame(
    frame: &[u8],
    source: DataLinkAddress,
    port_index: usize,
    ingress_network: u16,
    table: &RwLock<RoutingTable>,
    ports: &[(u16, Arc<dyn RouterDataLink>)],
) -> Result<(), DataLinkError> {
    let mut r = Reader::new(frame);
    let npdu = Npdu::decode(&mut r).map_err(|_| DataLinkError::InvalidFrame)?;

    let is_network_msg = (npdu.control & 0x80) != 0;
    let pos = r.position();
    let remaining_payload = &frame[pos..];

    // Handle network-layer messages.
    if is_network_msg {
        if let Some(msg_type) = npdu.message_type {
            handle_network_message(
                msg_type,
                remaining_payload,
                source,
                port_index,
                ingress_network,
                table,
                ports,
            )
            .await;
        }
        return Ok(());
    }

    // Application-layer forwarding: check for DNET.
    let dnet = match npdu.destination {
        Some(ref dest) => dest.network,
        None => return Ok(()), // Local message — no forwarding needed.
    };

    // Check hop count.
    let hop_count = npdu.hop_count.unwrap_or(255);
    if hop_count == 0 {
        return Ok(()); // Discard — hop count exhausted.
    }

    if dnet == 0xFFFF {
        // Global broadcast — forward to all ports except ingress.
        let mut fwd_npdu = npdu;
        fwd_npdu.hop_count = Some(hop_count - 1);
        // Set SNET/SADR if not already set.
        if fwd_npdu.source.is_none() {
            fwd_npdu.source = Some(mac_from_address(ingress_network, &source));
        }

        let fwd_frame = encode_npdu_with_payload(&fwd_npdu, remaining_payload)?;

        for (i, (_, dl)) in ports.iter().enumerate() {
            if i != port_index {
                let _ = dl
                    .send_frame(DataLinkAddress::local_broadcast(47808), &fwd_frame)
                    .await;
            }
        }
    } else {
        // Unicast/directed broadcast — look up route.
        let route = table.read().await.lookup(dnet);
        match route {
            Some(RouteEntry::Direct(target_port)) => {
                if target_port == port_index {
                    return Ok(()); // Don't forward back to ingress.
                }
                let mut fwd_npdu = npdu;
                fwd_npdu.hop_count = Some(hop_count - 1);
                if fwd_npdu.source.is_none() {
                    fwd_npdu.source = Some(mac_from_address(ingress_network, &source));
                }

                let fwd_frame = encode_npdu_with_payload(&fwd_npdu, remaining_payload)?;

                // If DADR has mac_len > 0, send to that MAC; otherwise broadcast on the target network.
                let target_addr = match &fwd_npdu.destination {
                    Some(dest) if dest.mac_len > 0 => {
                        // For simplicity, broadcast on target network.
                        // A full implementation would reconstruct the target DataLinkAddress from the MAC.
                        DataLinkAddress::local_broadcast(47808)
                    }
                    _ => DataLinkAddress::local_broadcast(47808),
                };

                let _ = ports[target_port]
                    .1
                    .send_frame(target_addr, &fwd_frame)
                    .await;
            }
            Some(RouteEntry::Learned(target_port, via)) => {
                if target_port == port_index {
                    return Ok(());
                }
                let mut fwd_npdu = npdu;
                fwd_npdu.hop_count = Some(hop_count - 1);
                if fwd_npdu.source.is_none() {
                    fwd_npdu.source = Some(mac_from_address(ingress_network, &source));
                }

                let fwd_frame = encode_npdu_with_payload(&fwd_npdu, remaining_payload)?;
                let _ = ports[target_port].1.send_frame(via, &fwd_frame).await;
            }
            None => {
                // No route — discard.
                log::debug!("router: no route to DNET {dnet}");
            }
        }
    }

    Ok(())
}

async fn handle_network_message(
    msg_type: u8,
    payload: &[u8],
    _source: DataLinkAddress,
    port_index: usize,
    ingress_network: u16,
    table: &RwLock<RoutingTable>,
    ports: &[(u16, Arc<dyn RouterDataLink>)],
) {
    match msg_type {
        network_msg::WHO_IS_ROUTER_TO_NETWORK => {
            // Respond with I-Am-Router-To-Network listing all our networks.
            let networks = table.read().await.all_reachable_networks();
            let response_payload = network_msg::encode_i_am_router(&networks);

            // Build NPDU with network message type.
            let mut npdu = Npdu::new(0x80); // network-layer message
            npdu.message_type = Some(network_msg::I_AM_ROUTER_TO_NETWORK);

            if let Ok(frame) = encode_npdu_with_payload(&npdu, &response_payload) {
                let _ = ports[port_index]
                    .1
                    .send_frame(DataLinkAddress::local_broadcast(47808), &frame)
                    .await;
            }
        }
        network_msg::I_AM_ROUTER_TO_NETWORK => {
            // Learn routes from the advertising router.
            let networks = network_msg::decode_network_list(payload);
            let mut tbl = table.write().await;
            for net in networks {
                if net != ingress_network {
                    tbl.learn_route(net, port_index, _source);
                }
            }
        }
        _ => {
            log::debug!(
                "router: unhandled network message type 0x{msg_type:02x} on port {port_index}"
            );
        }
    }
}

/// Construct an NpduAddress from a DataLinkAddress for SNET/SADR encoding.
fn mac_from_address(network: u16, addr: &DataLinkAddress) -> NpduAddress {
    match addr {
        DataLinkAddress::Ip(sock) => {
            let mut mac = [0u8; 18];
            match sock.ip() {
                std::net::IpAddr::V4(v4) => {
                    mac[..4].copy_from_slice(&v4.octets());
                    mac[4..6].copy_from_slice(&sock.port().to_be_bytes());
                    NpduAddress {
                        network,
                        mac,
                        mac_len: 6,
                    }
                }
                std::net::IpAddr::V6(v6) => {
                    mac[..16].copy_from_slice(&v6.octets());
                    mac[16..18].copy_from_slice(&sock.port().to_be_bytes());
                    NpduAddress {
                        network,
                        mac,
                        mac_len: 18,
                    }
                }
            }
        }
        DataLinkAddress::Mstp(m) => {
            let mut mac = [0u8; 18];
            mac[0] = *m;
            NpduAddress {
                network,
                mac,
                mac_len: 1,
            }
        }
    }
}

/// Re-encode an NPDU header with a payload appended.
fn encode_npdu_with_payload(npdu: &Npdu, payload: &[u8]) -> Result<Vec<u8>, DataLinkError> {
    let mut buf = [0u8; 1500];
    let mut w = Writer::new(&mut buf);
    npdu.encode(&mut w)
        .map_err(|_| DataLinkError::InvalidFrame)?;
    w.write_all(payload)
        .map_err(|_| DataLinkError::FrameTooLarge)?;
    Ok(w.as_written().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// A mock DataLink that records sent frames and can inject received frames.
    #[derive(Clone)]
    struct MockDataLink {
        sent: Arc<Mutex<Vec<(DataLinkAddress, Vec<u8>)>>>,
        recv_queue: Arc<Mutex<VecDeque<(Vec<u8>, DataLinkAddress)>>>,
    }

    impl MockDataLink {
        fn new() -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
                recv_queue: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        #[allow(dead_code)]
        fn push_recv(&self, data: Vec<u8>, source: DataLinkAddress) {
            self.recv_queue.lock().unwrap().push_back((data, source));
        }

        fn take_sent(&self) -> Vec<(DataLinkAddress, Vec<u8>)> {
            std::mem::take(&mut *self.sent.lock().unwrap())
        }
    }

    impl RouterDataLink for MockDataLink {
        fn send_frame<'a>(
            &'a self,
            address: DataLinkAddress,
            payload: &'a [u8],
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), DataLinkError>> + Send + 'a>,
        > {
            self.sent.lock().unwrap().push((address, payload.to_vec()));
            Box::pin(std::future::ready(Ok(())))
        }

        fn recv_frame<'a>(
            &'a self,
            buf: &'a mut [u8],
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<(usize, DataLinkAddress), DataLinkError>>
                    + Send
                    + 'a,
            >,
        > {
            if let Some((data, source)) = self.recv_queue.lock().unwrap().pop_front() {
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Box::pin(std::future::ready(Ok((n, source))))
            } else {
                // Return an error to avoid infinite loop in tests.
                Box::pin(std::future::ready(Err(DataLinkError::InvalidFrame)))
            }
        }
    }

    fn source_addr() -> DataLinkAddress {
        DataLinkAddress::Ip("10.0.0.1:47808".parse().unwrap())
    }

    #[test]
    fn routing_table_direct_init() {
        let dl = MockDataLink::new();
        let ports = vec![
            RouterPort {
                network: 1,
                datalink: Arc::new(dl.clone()),
            },
            RouterPort {
                network: 2,
                datalink: Arc::new(dl.clone()),
            },
        ];
        let router = BacnetRouter::new(ports);
        let rt = router.table.blocking_read();
        assert_eq!(rt.lookup(1), Some(RouteEntry::Direct(0)));
        assert_eq!(rt.lookup(2), Some(RouteEntry::Direct(1)));
    }

    #[tokio::test]
    async fn forward_frame_between_ports() {
        let dl_a = MockDataLink::new();
        let dl_b = MockDataLink::new();

        // Build a frame on port A with DNET=2 (port B).
        let mut npdu = Npdu::new(0x20); // has destination
        npdu.destination = Some(NpduAddress {
            network: 2,
            mac: [0u8; 18],
            mac_len: 0,
        });
        npdu.hop_count = Some(255);

        let app_payload = [0xAA, 0xBB, 0xCC];
        let frame = encode_npdu_with_payload(&npdu, &app_payload).unwrap();

        // Handle the frame as if it arrived on port 0 (network 1).
        let table = Arc::new(RwLock::new(RoutingTable::new()));
        {
            let mut t = table.write().await;
            t.add_direct_route(1, 0);
            t.add_direct_route(2, 1);
        }

        let ports: Vec<(u16, Arc<dyn RouterDataLink>)> =
            vec![(1, Arc::new(dl_a.clone())), (2, Arc::new(dl_b.clone()))];

        handle_frame(&frame, source_addr(), 0, 1, &table, &ports)
            .await
            .unwrap();

        // Port B should have received the forwarded frame.
        let sent_b = dl_b.take_sent();
        assert_eq!(sent_b.len(), 1, "frame should be forwarded to port B");

        // Decode the forwarded frame to verify hop count was decremented.
        let mut r = Reader::new(&sent_b[0].1);
        let fwd = Npdu::decode(&mut r).unwrap();
        assert_eq!(fwd.hop_count, Some(254));
        // SNET should be set to ingress network.
        assert!(fwd.source.is_some());
        assert_eq!(fwd.source.unwrap().network, 1);
    }

    #[tokio::test]
    async fn broadcast_forwarding() {
        let dl_a = MockDataLink::new();
        let dl_b = MockDataLink::new();
        let dl_c = MockDataLink::new();

        let mut npdu = Npdu::new(0x20);
        npdu.destination = Some(NpduAddress {
            network: 0xFFFF,
            mac: [0u8; 18],
            mac_len: 0,
        });
        npdu.hop_count = Some(255);

        let frame = encode_npdu_with_payload(&npdu, &[0x01]).unwrap();

        let table = Arc::new(RwLock::new(RoutingTable::new()));
        {
            let mut t = table.write().await;
            t.add_direct_route(1, 0);
            t.add_direct_route(2, 1);
            t.add_direct_route(3, 2);
        }

        let ports: Vec<(u16, Arc<dyn RouterDataLink>)> = vec![
            (1, Arc::new(dl_a.clone())),
            (2, Arc::new(dl_b.clone())),
            (3, Arc::new(dl_c.clone())),
        ];

        // Frame arrives on port 0 — should be forwarded to ports 1 and 2 but NOT back to 0.
        handle_frame(&frame, source_addr(), 0, 1, &table, &ports)
            .await
            .unwrap();

        assert!(
            dl_a.take_sent().is_empty(),
            "should not forward back to ingress"
        );
        assert_eq!(dl_b.take_sent().len(), 1);
        assert_eq!(dl_c.take_sent().len(), 1);
    }

    #[tokio::test]
    async fn hop_count_zero_discards() {
        let dl_a = MockDataLink::new();
        let dl_b = MockDataLink::new();

        let mut npdu = Npdu::new(0x20);
        npdu.destination = Some(NpduAddress {
            network: 2,
            mac: [0u8; 18],
            mac_len: 0,
        });
        npdu.hop_count = Some(0); // exhausted

        let frame = encode_npdu_with_payload(&npdu, &[0x01]).unwrap();

        let table = Arc::new(RwLock::new(RoutingTable::new()));
        {
            let mut t = table.write().await;
            t.add_direct_route(1, 0);
            t.add_direct_route(2, 1);
        }

        let ports: Vec<(u16, Arc<dyn RouterDataLink>)> =
            vec![(1, Arc::new(dl_a.clone())), (2, Arc::new(dl_b.clone()))];

        handle_frame(&frame, source_addr(), 0, 1, &table, &ports)
            .await
            .unwrap();

        assert!(
            dl_b.take_sent().is_empty(),
            "hop count 0 should be discarded"
        );
    }

    #[tokio::test]
    async fn i_am_router_learns_routes() {
        let table = Arc::new(RwLock::new(RoutingTable::new()));
        {
            let mut t = table.write().await;
            t.add_direct_route(1, 0);
        }

        let networks = network_msg::encode_i_am_router(&[10, 20]);

        let mut npdu = Npdu::new(0x80);
        npdu.message_type = Some(network_msg::I_AM_ROUTER_TO_NETWORK);

        let frame = encode_npdu_with_payload(&npdu, &networks).unwrap();

        let dl = MockDataLink::new();
        let ports: Vec<(u16, Arc<dyn RouterDataLink>)> = vec![(1, Arc::new(dl.clone()))];

        handle_frame(&frame, source_addr(), 0, 1, &table, &ports)
            .await
            .unwrap();

        let tbl = table.read().await;
        assert!(tbl.lookup(10).is_some());
        assert!(tbl.lookup(20).is_some());
    }
}
