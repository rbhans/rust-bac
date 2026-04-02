//! BACnet MS/TP data link layer implementation.
//!
//! Implements the ASHRAE 135 Clause 9 master node state machine for
//! MS/TP (Master-Slave/Token-Passing) over RS-485 serial links.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(async_fn_in_trait)]

#[cfg(all(feature = "alloc", not(feature = "std")))]
extern crate alloc;

/// CRC-8 and CRC-16 routines for MS/TP frame verification.
pub mod crc;
/// MS/TP frame encoding and decoding.
#[cfg(feature = "alloc")]
pub mod frame;
#[cfg(feature = "std")]
mod transport;

#[cfg(feature = "std")]
pub use transport::{MstpConfig, MstpTransport};
