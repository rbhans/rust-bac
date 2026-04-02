//! BACnet network-layer message constants and codec.
//!
//! Implements encoding and decoding for the network-layer messages
//! defined in ASHRAE 135 Clause 6.6.3.

/// Who-Is-Router-To-Network (0x00).
pub const WHO_IS_ROUTER_TO_NETWORK: u8 = 0x00;
/// I-Am-Router-To-Network (0x01).
pub const I_AM_ROUTER_TO_NETWORK: u8 = 0x01;
/// I-Could-Be-Router-To-Network (0x02).
pub const I_COULD_BE_ROUTER_TO_NETWORK: u8 = 0x02;
/// Reject-Message-To-Network (0x03).
pub const REJECT_MESSAGE_TO_NETWORK: u8 = 0x03;
/// Router-Busy-To-Network (0x04).
pub const ROUTER_BUSY_TO_NETWORK: u8 = 0x04;
/// Router-Available-To-Network (0x05).
pub const ROUTER_AVAILABLE_TO_NETWORK: u8 = 0x05;
/// Initialize-Routing-Table (0x06).
pub const INITIALIZE_ROUTING_TABLE: u8 = 0x06;
/// Initialize-Routing-Table-Ack (0x07).
pub const INITIALIZE_ROUTING_TABLE_ACK: u8 = 0x07;
/// Establish-Connection-To-Network (0x08).
pub const ESTABLISH_CONNECTION_TO_NETWORK: u8 = 0x08;
/// Disconnect-Connection-To-Network (0x09).
pub const DISCONNECT_CONNECTION_TO_NETWORK: u8 = 0x09;

/// Encode an I-Am-Router-To-Network payload.
///
/// The payload is simply a list of 16-bit network numbers in big-endian order.
pub fn encode_i_am_router(networks: &[u16]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(networks.len() * 2);
    for &net in networks {
        buf.extend_from_slice(&net.to_be_bytes());
    }
    buf
}

/// Decode a list of network numbers from a network-layer message payload.
///
/// Used for I-Am-Router-To-Network, Who-Is-Router-To-Network, etc.
pub fn decode_network_list(payload: &[u8]) -> Vec<u16> {
    payload
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let networks = vec![1, 2, 100, 65535];
        let encoded = encode_i_am_router(&networks);
        assert_eq!(encoded.len(), 8);
        let decoded = decode_network_list(&encoded);
        assert_eq!(decoded, networks);
    }

    #[test]
    fn decode_empty_payload() {
        let decoded = decode_network_list(&[]);
        assert!(decoded.is_empty());
    }

    #[test]
    fn decode_single_network() {
        let encoded = 42u16.to_be_bytes();
        let decoded = decode_network_list(&encoded);
        assert_eq!(decoded, vec![42]);
    }
}
