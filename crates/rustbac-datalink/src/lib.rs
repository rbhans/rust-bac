//! BACnet data-link layer abstraction and BACnet/IP transport.
//!
//! Provides the [`DataLink`] trait for sending and receiving BACnet frames,
//! along with a ready-to-use [`BacnetIpTransport`] (BACnet/IP over UDP)
//! including BBMD foreign-device registration support.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(async_fn_in_trait)]

/// Network-level addressing for BACnet data-link endpoints.
pub mod address;
/// BACnet/IP (Annex J) transport implementation.
#[cfg(feature = "std")]
pub mod bip;
/// BACnet/IPv6 (Annex U) transport implementation.
#[cfg(feature = "std")]
pub mod bip6;
/// PCAP packet capture via a [`DataLink`] wrapper.
#[cfg(feature = "std")]
pub mod capture;
/// The [`DataLink`] trait and associated error type.
pub mod traits;

pub use address::DataLinkAddress;
#[cfg(feature = "std")]
pub use bip::transport::{BacnetIpTransport, BroadcastDistributionEntry, ForeignDeviceTableEntry};
#[cfg(feature = "std")]
pub use bip6::transport::BacnetIp6Transport;
#[cfg(feature = "std")]
pub use capture::CapturingDataLink;
pub use traits::DataLinkError;
#[cfg(feature = "std")]
pub use traits::DataLink;
