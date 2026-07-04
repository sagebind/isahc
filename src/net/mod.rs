//! Types for configuring how HTTP connections interact with the network.

use crate::config::setopt::{EasyHandle, SetOpt, SetOptError};

mod dial;
pub mod dns;
pub mod interface;
pub mod rewrite;

pub use dial::{Dialer, DialerParseError};

/// Supported IP versions that can be used.
#[derive(Clone, Debug)]
pub enum IpVersion {
    /// Use IPv4 addresses only. IPv6 addresses will be ignored.
    V4,

    /// Use IPv6 addresses only. IPv4 addresses will be ignored.
    V6,

    /// Use either IPv4 or IPv6 addresses. By default IPv6 addresses are
    /// preferred if available, otherwise an IPv4 address will be used. IPv6
    /// addresses are tried first by following the recommendations of [RFC 6555
    /// "Happy Eyeballs"](https://tools.ietf.org/html/rfc6555).
    Any,
}

impl Default for IpVersion {
    fn default() -> Self {
        Self::Any
    }
}

impl SetOpt for IpVersion {
    fn set_opt(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        easy.ip_resolve(match &self {
            IpVersion::V4 => curl::easy::IpResolve::V4,
            IpVersion::V6 => curl::easy::IpResolve::V6,
            IpVersion::Any => curl::easy::IpResolve::Any,
        })?;

        Ok(())
    }
}
