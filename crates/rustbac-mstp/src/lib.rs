//! BACnet MS/TP data link layer implementation.
//!
//! Implements the ASHRAE 135 Clause 9 master node state machine for
//! MS/TP (Master-Slave/Token-Passing) over RS-485 serial links.

#![allow(async_fn_in_trait)]

mod crc;
mod frame;
mod transport;

pub use transport::{MstpConfig, MstpTransport};
