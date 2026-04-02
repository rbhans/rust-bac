//! BACnet network router.
//!
//! Provides a multi-port BACnet router that forwards NPDUs between
//! networks according to ASHRAE 135 Clause 6.

pub mod network_msg;
pub mod router;
pub mod routing_table;

pub use router::{BacnetRouter, RouterDataLink, RouterPort};
pub use routing_table::RoutingTable;
