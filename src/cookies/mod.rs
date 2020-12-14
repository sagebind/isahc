//! Types for cookie state management.
//!
//! # Cookie jars
//!
//! By default HTTP is a mostly stateless protocol, but using cookies allow a
//! server to request a client to persist specific state between requests. Isahc
//! does not do this by default, but provides support for cookie state using a
//! _cookie jar_, which can store a list of cookies in memory between requests
//! and is responsible for keeping track of which cookies belong to which
//! domains.
//!
//! A cookie jar can be used on a per-request basis or for all requests sent via
//! a particular HTTP client. You can assign a cookie jar to a request or to a
//! client via the
//! [`Configurable::cookie_jar`](crate::config::Configurable::cookie_jar)
//! extension method. If different cookie jars are assigned on both a request
//! and the client sending the request, the one assigned to the individual
//! request will take precedence.
//!
//! The global default client instance does not have an assigned cookie jar.
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
    cookie::Cookie,
    jar::CookieJar,
};
