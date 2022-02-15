//! Configuration of DNS resolution.

use super::SetOpt;
use curl::easy::Easy2;
use std::{net::IpAddr, time::Duration};

/// DNS caching configuration.
///
/// The default configuration is for caching to be enabled with a 60 second
/// entry timeout.
///
/// See [`HttpClientBuilder::dns_cache`](crate::HttpClientBuilder::dns_cache)
/// for configuring a client's DNS cache.
#[derive(Clone, Debug)]
pub enum DnsCache {
    /// Disable DNS caching entirely.
    Disable,

    /// Enable DNS caching and keep entries in the cache for the given duration.
    Timeout(Duration),

    /// Enable DNS caching and cache entries forever.
    Forever,
}

impl Default for DnsCache {
    fn default() -> Self {
        // Match curl's default.
        Duration::from_secs(60).into()
    }
}

impl From<Duration> for DnsCache {
    fn from(duration: Duration) -> Self {
        DnsCache::Timeout(duration)
    }
}

impl SetOpt for DnsCache {
    #[allow(unsafe_code)]
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        let value = match self {
            DnsCache::Disable => 0,
            DnsCache::Timeout(duration) => duration.as_secs() as i64,
            DnsCache::Forever => -1,
        };

        // Use unsafe API, because safe API doesn't let us set to -1.
        unsafe {
            match curl_sys::curl_easy_setopt(easy.raw(), curl_sys::CURLOPT_DNS_CACHE_TIMEOUT, value)
            {
                curl_sys::CURLE_OK => Ok(()),
                code => Err(curl::Error::new(code)),
            }
        }
    }
}

/// A mapping of host and port pairs to IP addresses.
///
/// Entries added to this map can be used to override how DNS is resolved for a
/// request and use specific IP addresses instead of using the default name
/// resolver.
#[derive(Clone, Debug, Default)]
pub struct ResolveMap(Vec<String>);

impl ResolveMap {
    /// Create a new empty resolve map.
    pub const fn new() -> Self {
        ResolveMap(Vec::new())
    }

    /// Add a DNS mapping for a given host and port pair.
    #[must_use = "builders have no effect if unused"]
    pub fn add<H, A>(mut self, host: H, port: u16, addr: A) -> Self
    where
        H: AsRef<str>,
        A: Into<IpAddr>,
    {
        self.0
            .push(format!("{}:{}:{}", host.as_ref(), port, addr.into()));
        self
    }
}

impl SetOpt for ResolveMap {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        let mut list = curl::easy::List::new();

        for entry in self.0.iter() {
            list.append(entry)?;
        }

        easy.resolve(list)
    }
}
