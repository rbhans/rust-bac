//! BACnet data-link layer abstraction and BACnet/IP transport.
//!
//! Provides the [`DataLink`] trait for sending and receiving BACnet frames,
//! along with a ready-to-use [`BacnetIpTransport`] (BACnet/IP over UDP)
//! including BBMD foreign-device registration support.

#![allow(async_fn_in_trait)]

/// Network-level addressing for BACnet data-link endpoints.
pub mod address;
/// BACnet/IP (Annex J) transport implementation.
pub mod bip;
/// PCAP packet capture via a [`DataLink`] wrapper.
pub mod capture;
/// The [`DataLink`] trait and associated error type.
pub mod traits;

pub use address::DataLinkAddress;
pub use bip::transport::{BacnetIpTransport, BroadcastDistributionEntry, ForeignDeviceTableEntry};
pub use capture::CapturingDataLink;
pub use traits::{DataLink, DataLinkError};
