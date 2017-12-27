use http::{self, Uri};
use std::time::Duration;


/// Defines various protocol and connection options.
#[derive(Clone, Debug)]
pub struct Options {
    /// The policy for automatically following server redirects.
    ///
    /// The default is to not follow redirects.
    pub redirect_policy: RedirectPolicy,

    /// A preferred HTTP version the client should attempt to use to communicate to the server with.
    ///
    /// This is treated as a suggestion. A different version may be used if the server does not support it or negotiates
    /// a different version.
    ///
    /// The default value is `None` (any version).
    pub preferred_http_version: Option<http::Version>,

    /// A timeout for the maximum time allowed for a request-response cycle.
    ///
    /// The default value is `None` (unlimited).
    pub timeout: Option<Duration>,

    /// A timeout for the initial connection phase.
    ///
    /// The default value is 300 seconds.
    pub connect_timeout: Duration,

    /// Enable or disable TCP keepalive with a given probe interval.
    ///
    /// The default value is `None` (disabled).
    pub tcp_keepalive: Option<Duration>,

    /// Enable or disable the `TCP_NODELAY` option.
    ///
    /// The default value is `false`.
    pub tcp_nodelay: bool,

    /// Indicates whether the `Referer` header should be automatically updated.
    pub auto_referer: bool,

    /// A proxy to use for requests.
    ///
    /// The proxy protocol is specified by the URI scheme.
    ///
    /// - **`http`**: Proxy. Default when no scheme is specified.
    /// - **`https`**: HTTPS Proxy. (Added in 7.52.0 for OpenSSL, GnuTLS and NSS)
    /// - **`socks4`**: SOCKS4 Proxy.
    /// - **`socks4a`**: SOCKS4a Proxy. Proxy resolves URL hostname.
    /// - **`socks5`**: SOCKS5 Proxy.
    /// - **`socks5h`**: SOCKS5 Proxy. Proxy resolves URL hostname.
    pub proxy: Option<Uri>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            redirect_policy: RedirectPolicy::default(),
            preferred_http_version: None,
            timeout: None,
            connect_timeout: Duration::from_secs(300),
            tcp_keepalive: None,
            tcp_nodelay: false,
            auto_referer: false,
            proxy: None,
        }
    }
}

impl Options {
    pub fn new() -> Options {
        Options::default()
    }
}


/// Describes a policy for handling server redirects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPolicy {
    /// Do not apply any special treatment to redirect responses. The response will be return as-is and redirects will
    /// not be followed.
    ///
    /// This is the default policy.
    None,
    /// Follow all redirects automatically.
    Follow,
    /// Follow redirects automatically up to a maximum number of redirects.
    Limit(u32),
}

impl Default for RedirectPolicy {
    fn default() -> RedirectPolicy {
        RedirectPolicy::None
    }
}
