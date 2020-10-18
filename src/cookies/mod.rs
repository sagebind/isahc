//! Cookie state management.
//!
//! # Availability
//!
//! This module is only available when the [`cookies`](index.html#cookies)
//! feature is enabled.

mod cookie;
mod jar;
pub(crate) mod interceptor;

#[cfg(feature = "psl")]
mod psl;

pub use self::{
    cookie::{Cookie, ParseError},
    jar::*,
};
