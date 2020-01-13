//! Configuration of DNS resolution.

use super::SetOpt;
use curl::easy::Easy2;
use http::uri::Authority;
use std::{
    iter::FromIterator,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

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
            match curl_sys::curl_easy_setopt(easy.raw(), curl_sys::CURLOPT_DNS_CACHE_TIMEOUT, value) {
                curl_sys::CURLE_OK => Ok(()),
                code => Err(curl::Error::new(code)),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct DnsMapping {
    authority: Authority,
    addr: IpAddr,
}

impl DnsMapping {
    pub fn new(authority: Authority, addr: IpAddr) -> Self {
        Self {
            authority,
            addr,
        }
    }
}

impl From<(Authority, IpAddr)> for DnsMapping {
    fn from((authority, addr): (Authority, IpAddr)) -> Self {
        Self::new(authority, addr)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mappings(Vec<String>);

impl FromIterator<DnsMapping> for Mappings {
    fn from_iter<I: IntoIterator<Item = DnsMapping>>(iter: I) -> Self {
        let mut mappings = Self::default();

        for mapping in iter {
            mappings = mappings.add(mapping.authority, mapping.addr);
        }

        mappings
    }
}

impl Mappings {
    pub fn add(mut self, authority: Authority, addr: IpAddr) -> Self {
        self.0.push(format!("{}:{}", authority, addr));
        self
    }
}

impl SetOpt for Mappings {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        let mut list = curl::easy::List::new();

        for entry in self.0.iter() {
            list.append(entry)?;
        }

        easy.resolve(list)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Servers(String);

impl FromIterator<SocketAddr> for Servers {
    fn from_iter<I: IntoIterator<Item = SocketAddr>>(iter: I) -> Self {
        Servers(iter.into_iter()
            .map(|addr| addr.to_string())
            .collect::<Vec<_>>()
            .join(","))
    }
}

impl SetOpt for Servers {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        // DNS servers should not be hard error.
        if let Err(e) = easy.dns_servers(&self.0) {
            log::warn!("DNS servers could not be configured: {}", e);
        }

        Ok(())
    }
}
