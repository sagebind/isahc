//! Provide the ability to rewrite host and port information for outgoing HTTP
//! connections.

#![expect(private_interfaces)]

use curl::easy::List;
use std::{
    fmt::{self, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

/// Contains an ordered list of rewrite rules to apply to outgoing HTTP
/// connections.
#[derive(Clone, Debug)]
pub struct RuleSet {
    list: Arc<List>,
}

impl RuleSet {
    /// Create a new rule set builder.
    pub fn builder() -> Builder {
        Builder::default()
    }
}

/// A builder for constructing a set of rewrite rules to apply to outgoing HTTP
/// connections.
#[derive(Debug)]
pub struct Builder {
    list: List,
}

impl Builder {
    /// Add a rule to the rule set.
    ///
    /// Rules are applied in the order they are added.
    pub fn push<M: Matcher, D: Destination>(mut self, matcher: M, dest: D) -> Self {
        let rule = RuleFormatter(matcher, dest).to_string();
        self.list.append(&rule).unwrap();
        self
    }

    /// Build the rule set.
    pub fn build(self) -> RuleSet {
        RuleSet {
            list: Arc::new(self.list),
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self { list: List::new() }
    }
}

/// A matcher that matches all outgoing requests.
#[derive(Clone, Copy, Debug)]
pub struct Any;

/// Matches outgoing HTTP requests based on their destination host and/or port.
pub trait Matcher {
    #[doc(hidden)]
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result;
}

impl Matcher for Any {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result {
        f.write_char(':')
    }
}

impl Matcher for SocketAddr {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result {
        write!(f, "{}:{}", self.ip(), self.port())
    }
}

impl Matcher for IpAddr {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result {
        write!(f, "{}:", self)
    }
}

/// A destination which requests should be rewritten to.
pub trait Destination {
    #[doc(hidden)]
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result;
}

impl Destination for SocketAddr {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, sealed: Sealed) -> fmt::Result {
        Destination::write_format(&self.ip(), f, sealed)?;
        fmt::Display::fmt(&self.port(), f)
    }
}

impl Destination for IpAddr {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, sealed: Sealed) -> fmt::Result {
        match self {
            IpAddr::V4(addr) => addr.write_format(f, sealed),
            IpAddr::V6(addr) => addr.write_format(f, sealed),
        }
    }
}

impl Destination for Ipv4Addr {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result {
        write!(f, "{}:", self)
    }
}

impl Destination for Ipv6Addr {
    fn write_format(&self, f: &mut fmt::Formatter<'_>, _: Sealed) -> fmt::Result {
        write!(f, "[{}]:", self)
    }
}

struct RuleFormatter<M: Matcher, D: Destination>(M, D);

impl<M: Matcher, D: Destination> fmt::Display for RuleFormatter<M, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.write_format(f, Sealed)?;
        f.write_char(':')?;
        self.1.write_format(f, Sealed)
    }
}

/// Private marker to seal the our trait methods and prevent external
/// implementations.
struct Sealed;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_rewrite_any_to_localhost_ipv4() {
        let rule = RuleFormatter(Any, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080));

        assert_eq!(rule.to_string(), "::127.0.0.1:8080");
    }

    #[test]
    fn format_rewrite_any_to_localhost_ipv6() {
        let rule = RuleFormatter(Any, SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 8080));

        assert_eq!(rule.to_string(), "::[::1]:8080");
    }

    #[test]
    fn format_rewrite_any_to_localhost_same_port_ipv6() {
        let rule = RuleFormatter(Any, Ipv6Addr::LOCALHOST);

        assert_eq!(rule.to_string(), "::[::1]:");
    }
}
