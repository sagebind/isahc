//! Definition of all client and request configuration options.
//!
//! Individual options are separated out into multiple types. Each type acts
//! both as a "field name" and the value of that option.

// Options are implemented as structs of various kinds that can be "applied" to
// a curl easy handle. This helps to reduce code duplication as there are many
// supported options, and also helps avoid having a massive function that does
// all the configuring.
//
// When adding new config options, remember to add methods for setting the
// option both in HttpClientBuilder and RequestBuilderExt. In addition, be sure
// to update the client code to apply the option when configuring an easy
// handle.

use curl::easy::Easy2;
use std::{
    iter::FromIterator,
    net::SocketAddr,
    time::Duration,
};

pub(crate) mod ssl;

pub use ssl::{
    ClientCertificate,
    CaCertificate,
    PrivateKey,
    SslOption,
};

/// A helper trait for applying a configuration value to a given curl handle.
pub(crate) trait SetOpt {
    /// Apply this configuration option to the given curl handle.
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error>;
}

impl SetOpt for http::HeaderMap {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        let mut headers = curl::easy::List::new();

        for (name, value) in self.iter() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header)?;
        }

        easy.http_headers(headers)
    }
}

/// Describes a policy for handling server redirects.
///
/// The default is to not follow redirects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPolicy {
    /// Do not apply any special treatment to redirect responses. The response
    /// will be returned as-is and redirects will not be followed.
    ///
    /// This is the default policy.
    None,
    /// Follow all redirects automatically.
    Follow,
    /// Follow redirects automatically up to a maximum number of redirects.
    Limit(u32),
}

impl Default for RedirectPolicy {
    fn default() -> Self {
        RedirectPolicy::None
    }
}

impl SetOpt for RedirectPolicy {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match self {
            RedirectPolicy::Follow => {
                easy.follow_location(true)?;
            }
            RedirectPolicy::Limit(max) => {
                easy.follow_location(true)?;
                easy.max_redirections(*max)?;
            }
            RedirectPolicy::None => {
                easy.follow_location(false)?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Timeout(pub(crate) Duration);

impl SetOpt for Timeout {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.timeout(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ConnectTimeout(pub(crate) Duration);

impl SetOpt for ConnectTimeout {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.connect_timeout(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TcpKeepAlive(pub(crate) Duration);

impl SetOpt for TcpKeepAlive {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.tcp_keepalive(true)?;
        easy.tcp_keepintvl(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PreferredHttpVersion(pub(crate) http::Version);

impl SetOpt for PreferredHttpVersion {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.http_version(match self.0 {
            http::Version::HTTP_10 => curl::easy::HttpVersion::V10,
            http::Version::HTTP_11 => curl::easy::HttpVersion::V11,
            http::Version::HTTP_2 => curl::easy::HttpVersion::V2,
            _ => curl::easy::HttpVersion::Any,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TcpNoDelay;

impl SetOpt for TcpNoDelay {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.tcp_nodelay(true)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AutoReferer;

impl SetOpt for AutoReferer {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.autoreferer(true)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxUploadSpeed(pub(crate) u64);

impl SetOpt for MaxUploadSpeed {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.max_send_speed(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxDownloadSpeed(pub(crate) u64);

impl SetOpt for MaxDownloadSpeed {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.max_recv_speed(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DnsServers(pub(crate) Vec<SocketAddr>);

impl FromIterator<SocketAddr> for DnsServers {
    fn from_iter<I: IntoIterator<Item = SocketAddr>>(iter: I) -> Self {
        DnsServers(Vec::from_iter(iter))
    }
}

impl SetOpt for DnsServers {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        let dns_string = self.0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        // DNS servers should not be hard error.
        if let Err(e) = easy.dns_servers(&dns_string) {
            log::warn!("DNS servers could not be configured: {}", e);
        }

        Ok(())
    }
}

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

/// Decorator for marking certain configurations to apply to a proxy rather than
/// the origin itself.
#[derive(Clone, Debug)]
pub(crate) struct Proxy<T>(pub(crate) T);

/// Proxy URI specifies the type and host of a proxy to use.
impl SetOpt for Proxy<Option<http::Uri>> {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match &self.0 {
            Some(uri) => easy.proxy(&format!("{}", uri)),
            None => easy.proxy(""),
        }
    }
}

/// A list of host names that do not require a proxy to get reached, even if one
/// is specified.
///
/// See
/// [`HttpClientBuilder::proxy_blacklist`](crate::HttpClientBuilder::proxy_blacklist)
/// for configuring a client's no proxy list.
#[derive(Clone, Debug)]
pub(crate) struct ProxyBlacklist {
    skip: String,
}

impl FromIterator<String> for ProxyBlacklist {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self {
            skip: iter.into_iter().collect::<Vec<_>>().join(","),
        }
    }
}

impl SetOpt for ProxyBlacklist {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.noproxy(&self.skip)
    }
}

/// Close the connection when the request completes instead of returning it to
/// the connection cache.
#[derive(Clone, Debug)]
pub(crate) struct CloseConnection(pub(crate) bool);

impl SetOpt for CloseConnection {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.forbid_reuse(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EnableMetrics(pub(crate) bool);

impl SetOpt for EnableMetrics {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.progress(self.0)
    }
}
