//! Helpers for sending form bodies.

#[cfg(feature = "multipart")]
mod multipart;

#[cfg(feature = "multipart")]
pub use multipart::*;
