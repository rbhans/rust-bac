//! BACnet protocol encoding and decoding in pure Rust.
//!
//! `rustbac-core` provides zero-copy, `no_std`-compatible encoding and decoding
//! of BACnet APDUs, NPDUs, and service payloads. It forms the foundation of the
//! rustbac crate family and can be used standalone in embedded or constrained
//! environments.
//!
//! # Feature flags
//!
//! - **`std`** (default) — enables `std::error::Error` implementations.
//! - **`alloc`** (default) — enables service decoders that allocate (e.g. RPM, COV).
//! - **`serde`** — derives `Serialize`/`Deserialize` on core types.
//! - **`defmt`** — derives `defmt::Format` for embedded logging.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

/// APDU (Application Protocol Data Unit) types for confirmed/unconfirmed requests and responses.
pub mod apdu;
/// Binary encoding primitives, tag system, and zero-copy reader/writer.
pub mod encoding;
/// Error types for encoding and decoding operations.
pub mod error;
/// NPDU (Network Protocol Data Unit) encoding and decoding.
pub mod npdu;
/// BACnet service request and response codecs.
pub mod services;
/// Core BACnet data types: object identifiers, property identifiers, and data values.
pub mod types;

pub use error::{DecodeError, EncodeError};
