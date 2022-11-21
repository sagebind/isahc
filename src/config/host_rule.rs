//! Configuration of host rule.
use super::SetOpt;

/// A mapping of host and port pairs to specified host
///
/// Entries added to this map can be used to override how host is resolved for a
/// request and use specific host instead of using the default name
/// resolver.
#[derive(Clone, Debug, Default)]
pub struct HostRuleMap(Vec<String>);

impl HostRuleMap {
    /// Create a new empty rule map.
    pub const fn new() -> Self {
        HostRuleMap(Vec::new())
    }

    /// Add a host mapping for a given host and port pair.
    pub fn add<H>(mut self, host: H, port: u16, connect_to_host: H) -> Self
    where
        H: AsRef<str>,
    {
        self.0
            .push(format!("{}:{}:{}", host.as_ref(), port, connect_to_host.as_ref()));
        self
    }
}

impl SetOpt for HostRuleMap {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        let mut list = curl::easy::List::new();

        for entry in self.0.iter() {
            list.append(entry)?;
        }

        easy.connect_to(list)
    }
}
