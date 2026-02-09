#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod apdu;
pub mod encoding;
pub mod error;
pub mod npdu;
pub mod services;
pub mod types;

pub use error::{DecodeError, EncodeError};
