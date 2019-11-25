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
    time::Duration,
};

pub(crate) mod dns;
pub(crate) mod ssl;

pub use dns::DnsCache;
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

/// A strategy for selecting what HTTP versions should be used when
/// communicating with a server.
#[derive(Clone, Debug)]
pub struct VersionNegotiation {
    flag: curl::easy::HttpVersion,
    strict: bool,
}

impl Default for VersionNegotiation {
    fn default() -> Self {
        Self::latest_compatible()
    }
}

impl VersionNegotiation {
    /// Always prefer the latest supported version with a preference for old
    /// versions if necessary in order to connect. This is the default.
    ///
    /// Typically negotiation will begin with an HTTP/1.1 request, upgrading to
    /// HTTP/2 if possible, then to HTTP/3 if possible, etc.
    pub const fn latest_compatible() -> Self {
        Self {
            // In curl land, this basically the most lenient option. Alt-Svc is
            // used to upgrade to newer versions, and old versions are used if
            // the server doesn't respond to the HTTP/1.1 -> HTTP/2 upgrade.
            flag: curl::easy::HttpVersion::V2,
            strict: false,
        }
    }

    /// Connect via HTTP/1.0 and do not attempt to use a higher version.
    pub const fn http10() -> Self {
        Self {
            flag: curl::easy::HttpVersion::V10,
            strict: true,
        }
    }

    /// Connect via HTTP/1.1 and do not attempt to use a higher version.
    pub const fn http11() -> Self {
        Self {
            flag: curl::easy::HttpVersion::V11,
            strict: true,
        }
    }

    /// Connect via HTTP/2. Failure to connect will not fall back to old
    /// versions, unless HTTP/1.1 is negotiated via TLS ALPN before the session
    /// begins.
    ///
    /// If HTTP/2 support is not compiled in, then using this strategy will
    /// always result in an error.
    ///
    /// This strategy is often referred to as [HTTP/2 with Prior
    /// Knowledge](https://http2.github.io/http2-spec/#known-http).
    pub const fn http2() -> Self {
        Self {
            flag: curl::easy::HttpVersion::V2PriorKnowledge,
            strict: true,
        }
    }

    // /// Connect via HTTP/3. Failure to connect will not fall back to old
    // /// versions.
    // pub const fn http3() -> Self {
    //     Self {
    //         flag: curl::easy::HttpVersion::V3,
    //         strict: true,
    //     }
    // }
}

impl SetOpt for VersionNegotiation {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        if let Err(e) = easy.http_version(self.flag) {
            if self.strict {
                return Err(e);
            } else {
                log::debug!("failed to set HTTP version: {}", e);
            }
        }

        Ok(())
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
