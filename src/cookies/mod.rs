//! Cookie state management.
//!
//! This module provides a cookie jar implementation conforming to RFC 6265.
//!
//! Everything in this module requires the `cookies` feature to be enabled.

mod cookie;
mod jar;
pub(crate) mod middleware;
mod response;

#[cfg(feature = "psl")]
mod psl;

pub use self::{
    cookie::Cookie,
    jar::*,
};
