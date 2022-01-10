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

use self::{proxy::Proxy, request::SetOpt};
use crate::{
    auth::{Authentication, Credentials},
    is_http_version_supported,
};
use curl::easy::Easy2;
use std::{net::IpAddr, time::Duration};

pub(crate) mod client;
pub(crate) mod dial;
pub(crate) mod dns;
pub(crate) mod proxy;
pub(crate) mod redirect;
pub(crate) mod request;
pub(crate) mod ssl;

pub use dial::{Dialer, DialerParseError};
pub use dns::{DnsCache, ResolveMap};
pub use redirect::RedirectPolicy;
pub use ssl::{CaCertificate, ClientCertificate, PrivateKey, SslOption};

/// Provides additional methods when building a request for configuring various
/// execution-related options on how the request should be sent.
///
/// This trait can be used to either configure requests individually by invoking
/// them on an [`http::request::Builder`], or to configure the default settings
/// for an [`HttpClient`](crate::HttpClient) by invoking them on an
/// [`HttpClientBuilder`](crate::HttpClientBuilder).
///
/// This trait is sealed and cannot be implemented for types outside of Isahc.
pub trait Configurable: request::WithRequestConfig {
    /// Specify a maximum amount of time that a complete request/response cycle
    /// is allowed to take before being aborted. This includes DNS resolution,
    /// connecting to the server, writing the request, and reading the response.
    ///
    /// Since response bodies are streamed, you will likely receive a
    /// [`Response`](crate::http::Response) before the response body stream has
    /// been fully consumed. This means that the configured timeout will still
    /// be active for that request, and if it expires, further attempts to read
    /// from the stream will return a [`TimedOut`](std::io::ErrorKind::TimedOut)
    /// I/O error.
    ///
    /// This also means that if you receive a response with a body but do not
    /// immediately start reading from it, then the timeout timer will still be
    /// active and may expire before you even attempt to read the body. Keep
    /// this in mind when consuming responses and consider handling the response
    /// body right after you receive it if you are using this option.
    ///
    /// If not set, no timeout will be enforced.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::{prelude::*, Request};
    /// use std::time::Duration;
    ///
    /// // This page is too slow and won't respond in time.
    /// let response = Request::get("https://httpbin.org/delay/10")
    ///     .timeout(Duration::from_secs(5))
    ///     .body(())?
    ///     .send()
    ///     .expect_err("page should time out");
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn timeout(self, timeout: Duration) -> Self {
        self.with_config(move |config| {
            config.timeout = Some(timeout);
        })
    }

    /// Set a timeout for establishing connections to a host.
    ///
    /// If not set, a default connect timeout of 300 seconds will be used.
    #[must_use = "builders have no effect if unused"]
    fn connect_timeout(self, timeout: Duration) -> Self {
        self.with_config(move |config| {
            config.connect_timeout = Some(timeout);
        })
    }

    /// Specify a maximum amount of time where transfer rate can go below
    /// a minimum speed limit. `low_speed` is that limit in bytes/s.
    ///
    /// If not set, no low speed limits are imposed.
    #[must_use = "builders have no effect if unused"]
    fn low_speed_timeout(self, low_speed: u32, timeout: Duration) -> Self {
        self.with_config(move |config| {
            config.low_speed_timeout = Some((low_speed, timeout));
        })
    }

    /// Configure how the use of HTTP versions should be negotiated with the
    /// server.
    ///
    /// The default is [`VersionNegotiation::latest_compatible`].
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::{
    ///     config::VersionNegotiation,
    ///     prelude::*,
    ///     HttpClient,
    /// };
    ///
    /// // Never use anything newer than HTTP/1.x for this client.
    /// let http11_client = HttpClient::builder()
    ///     .version_negotiation(VersionNegotiation::http11())
    ///     .build()?;
    ///
    /// // HTTP/2 with prior knowledge.
    /// let http2_client = HttpClient::builder()
    ///     .version_negotiation(VersionNegotiation::http2())
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn version_negotiation(self, negotiation: VersionNegotiation) -> Self {
        self.with_config(move |config| {
            config.version_negotiation = Some(negotiation);
        })
    }

    /// Set a policy for automatically following server redirects.
    ///
    /// The default is to not follow redirects.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::{config::RedirectPolicy, prelude::*, Request};
    ///
    /// // This URL redirects us to where we want to go.
    /// let response = Request::get("https://httpbin.org/redirect/1")
    ///     .redirect_policy(RedirectPolicy::Follow)
    ///     .body(())?
    ///     .send()?;
    ///
    /// // This URL redirects too much!
    /// let error = Request::get("https://httpbin.org/redirect/10")
    ///     .redirect_policy(RedirectPolicy::Limit(5))
    ///     .body(())?
    ///     .send()
    ///     .expect_err("too many redirects");
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn redirect_policy(self, policy: RedirectPolicy) -> Self {
        self.with_config(move |config| {
            config.redirect_policy = Some(policy);
        })
    }

    /// Update the `Referer` header automatically when following redirects.
    #[must_use = "builders have no effect if unused"]
    fn auto_referer(self) -> Self {
        self.with_config(move |config| {
            config.auto_referer = Some(true);
        })
    }

    /// Set a cookie jar to use to accept, store, and supply cookies for
    /// incoming responses and outgoing requests.
    ///
    /// A cookie jar can be shared across multiple requests or with an entire
    /// client, allowing cookies to be persisted across multiple requests.
    ///
    /// # Availability
    ///
    /// This method is only available when the [`cookies`](index.html#cookies)
    /// feature is enabled.
    #[cfg(feature = "cookies")]
    #[must_use = "builders have no effect if unused"]
    fn cookie_jar(self, cookie_jar: crate::cookies::CookieJar) -> Self;

    /// Enable or disable automatic decompression of the response body for
    /// various compression algorithms as returned by the server in the
    /// [`Content-Encoding`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Encoding)
    /// response header.
    ///
    /// If set to true (the default), Isahc will automatically and transparently
    /// decode the HTTP response body for known and available compression
    /// algorithms. If the server returns a response with an unknown or
    /// unavailable encoding, Isahc will return an
    /// [`InvalidContentEncoding`](crate::error::ErrorKind::InvalidContentEncoding)
    /// error.
    ///
    /// If you do not specify a specific value for the
    /// [`Accept-Encoding`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Encoding)
    /// header, Isahc will set one for you automatically based on this option.
    #[must_use = "builders have no effect if unused"]
    fn automatic_decompression(self, decompress: bool) -> Self {
        self.with_config(move |config| {
            config.automatic_decompression = Some(decompress);
        })
    }

    /// Configure the use of the `Expect` request header when sending request
    /// bodies with HTTP/1.1.
    ///
    /// By default, when sending requests containing a body of large or unknown
    /// length over HTTP/1.1, Isahc will send the request headers first without
    /// the body and wait for the server to respond with a 100 (Continue) status
    /// code, as defined by [RFC 7231, Section
    /// 5.1.1](https://datatracker.ietf.org/doc/html/rfc7231#section-5.1.1).
    /// This gives the opportunity for the server to reject the response without
    /// needing to first transmit the request body over the network, if the body
    /// contents are not necessary for the server to determine an appropriate
    /// response.
    ///
    /// For servers that do not support this behavior and instead simply wait
    /// for the request body without responding with a 100 (Continue), there is
    /// a limited timeout before the response body is sent anyway without
    /// confirmation. The default timeout is 1 second, but this can be
    /// configured.
    ///
    /// The `Expect` behavior can also be disabled entirely.
    ///
    /// This configuration only takes effect when using HTTP/1.1.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use isahc::{
    ///     config::ExpectContinue,
    ///     prelude::*,
    ///     HttpClient,
    /// };
    ///
    /// // Use the default behavior (enabled).
    /// let client = HttpClient::builder()
    ///     .expect_continue(ExpectContinue::enabled())
    ///     // or equivalently...
    ///     .expect_continue(ExpectContinue::default())
    ///     // or equivalently...
    ///     .expect_continue(true)
    ///     .build()?;
    ///
    /// // Customize the timeout if the server doesn't respond with a 100
    /// // (Continue) status.
    /// let client = HttpClient::builder()
    ///     .expect_continue(ExpectContinue::timeout(Duration::from_millis(200)))
    ///     // or equivalently...
    ///     .expect_continue(Duration::from_millis(200))
    ///     .build()?;
    ///
    /// // Disable the Expect header entirely.
    /// let client = HttpClient::builder()
    ///     .expect_continue(ExpectContinue::disabled())
    ///     // or equivalently...
    ///     .expect_continue(false)
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn expect_continue<T>(self, expect: T) -> Self
    where
        T: Into<ExpectContinue>,
    {
        self.with_config(move |config| {
            config.expect_continue = Some(expect.into());
        })
    }

    /// Set one or more default HTTP authentication methods to attempt to use
    /// when authenticating with the server.
    ///
    /// Depending on the authentication schemes enabled, you will also need to
    /// set credentials to use for authentication using
    /// [`Configurable::credentials`].
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::{
    ///     auth::{Authentication, Credentials},
    ///     prelude::*,
    ///     HttpClient,
    /// };
    ///
    /// let client = HttpClient::builder()
    ///     .authentication(Authentication::basic() | Authentication::digest())
    ///     .credentials(Credentials::new("clark", "qwerty"))
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn authentication(self, authentication: Authentication) -> Self {
        self.with_config(move |config| {
            config.authentication = Some(authentication);
        })
    }

    /// Set the credentials to use for HTTP authentication.
    ///
    /// This setting will do nothing unless you also set one or more
    /// authentication methods using [`Configurable::authentication`].
    #[must_use = "builders have no effect if unused"]
    fn credentials(self, credentials: Credentials) -> Self {
        self.with_config(move |config| {
            config.credentials = Some(credentials);
        })
    }

    /// Enable TCP keepalive with a given probe interval.
    #[must_use = "builders have no effect if unused"]
    fn tcp_keepalive(self, interval: Duration) -> Self {
        self.with_config(move |config| {
            config.tcp_keepalive = Some(interval);
        })
    }

    /// Enables the `TCP_NODELAY` option on connect.
    #[must_use = "builders have no effect if unused"]
    fn tcp_nodelay(self) -> Self {
        self.with_config(move |config| {
            config.tcp_nodelay = Some(true);
        })
    }

    /// Bind local socket connections to a particular network interface.
    ///
    /// # Examples
    ///
    /// Bind to an IP address.
    ///
    /// ```
    /// use isahc::{
    ///     prelude::*,
    ///     config::NetworkInterface,
    ///     HttpClient,
    ///     Request,
    /// };
    /// use std::net::IpAddr;
    ///
    /// // Bind to an IP address.
    /// let client = HttpClient::builder()
    ///     .interface(IpAddr::from([192, 168, 1, 2]))
    ///     .build()?;
    ///
    /// // Bind to an interface by name (not supported on Windows).
    /// # #[cfg(unix)]
    /// let client = HttpClient::builder()
    ///     .interface(NetworkInterface::name("eth0"))
    ///     .build()?;
    ///
    /// // Reset to using whatever interface the TCP stack finds suitable (the
    /// // default).
    /// let request = Request::get("https://example.org")
    ///     .interface(NetworkInterface::any())
    ///     .body(())?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn interface<I>(self, interface: I) -> Self
    where
        I: Into<NetworkInterface>,
    {
        self.with_config(move |config| {
            config.interface = Some(interface.into());
        })
    }

    /// Select a specific IP version when resolving hostnames. If a given
    /// hostname does not resolve to an IP address of the desired version, then
    /// the request will fail with a connection error.
    ///
    /// This does not affect requests with an explicit IP address as the host.
    ///
    /// The default is [`IpVersion::Any`].
    #[must_use = "builders have no effect if unused"]
    fn ip_version(self, version: IpVersion) -> Self {
        self.with_config(move |config| {
            config.ip_version = Some(version);
        })
    }

    /// Specify a socket to connect to instead of the using the host and port
    /// defined in the request URI.
    ///
    /// # Examples
    ///
    /// Connecting to a Unix socket:
    ///
    /// ```
    /// use isahc::{
    ///     config::Dialer,
    ///     prelude::*,
    ///     Request,
    /// };
    ///
    /// # #[cfg(unix)]
    /// let request = Request::get("http://localhost/containers")
    ///     .dial(Dialer::unix_socket("/path/to/my.sock"))
    ///     .body(())?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    ///
    /// Connecting to a specific Internet socket address:
    ///
    /// ```
    /// use isahc::{
    ///     config::Dialer,
    ///     prelude::*,
    ///     Request,
    /// };
    /// use std::net::Ipv4Addr;
    ///
    /// let request = Request::get("http://exmaple.org")
    ///     // Actually issue the request to localhost on port 8080. The host
    ///     // header will remain unchanged.
    ///     .dial(Dialer::ip_socket((Ipv4Addr::LOCALHOST, 8080)))
    ///     .body(())?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn dial<D>(self, dialer: D) -> Self
    where
        D: Into<Dialer>,
    {
        self.with_config(move |config| {
            config.dial = Some(dialer.into());
        })
    }

    /// Set a proxy to use for requests.
    ///
    /// The proxy protocol is specified by the URI scheme.
    ///
    /// - **`http`**: Proxy. Default when no scheme is specified.
    /// - **`https`**: HTTPS Proxy. (Added in 7.52.0 for OpenSSL, GnuTLS and
    ///   NSS)
    /// - **`socks4`**: SOCKS4 Proxy.
    /// - **`socks4a`**: SOCKS4a Proxy. Proxy resolves URL hostname.
    /// - **`socks5`**: SOCKS5 Proxy.
    /// - **`socks5h`**: SOCKS5 Proxy. Proxy resolves URL hostname.
    ///
    /// By default no proxy will be used, unless one is specified in either the
    /// `http_proxy` or `https_proxy` environment variables.
    ///
    /// Setting to `None` explicitly disables the use of a proxy.
    ///
    /// # Examples
    ///
    /// Using `http://proxy:80` as a proxy:
    ///
    /// ```
    /// use isahc::{prelude::*, HttpClient};
    ///
    /// let client = HttpClient::builder()
    ///     .proxy(Some("http://proxy:80".parse()?))
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Explicitly disable the use of a proxy:
    ///
    /// ```
    /// use isahc::{prelude::*, HttpClient};
    ///
    /// let client = HttpClient::builder()
    ///     .proxy(None)
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn proxy(self, proxy: impl Into<Option<http::Uri>>) -> Self {
        self.with_config(move |config| {
            config.proxy = Some(proxy.into());
        })
    }

    /// Disable proxy usage for the provided list of hosts.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::{prelude::*, HttpClient};
    ///
    /// let client = HttpClient::builder()
    ///     // Disable proxy for specified hosts.
    ///     .proxy_blacklist(vec!["a.com", "b.org"])
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn proxy_blacklist<I, T>(self, hosts: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.with_config(move |config| {
            config.proxy_blacklist = Some(hosts.into_iter().map(T::into).collect());
        })
    }

    /// Set one or more HTTP authentication methods to attempt to use when
    /// authenticating with a proxy.
    ///
    /// Depending on the authentication schemes enabled, you will also need to
    /// set credentials to use for authentication using
    /// [`Configurable::proxy_credentials`].
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::{
    ///     auth::{Authentication, Credentials},
    ///     prelude::*,
    ///     HttpClient,
    /// };
    ///
    /// let client = HttpClient::builder()
    ///     .proxy("http://proxy:80".parse::<http::Uri>()?)
    ///     .proxy_authentication(Authentication::basic())
    ///     .proxy_credentials(Credentials::new("clark", "qwerty"))
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn proxy_authentication(self, authentication: Authentication) -> Self {
        self.with_config(move |config| {
            config.proxy_authentication = Some(Proxy(authentication));
        })
    }

    /// Set the credentials to use for proxy authentication.
    ///
    /// This setting will do nothing unless you also set one or more proxy
    /// authentication methods using
    /// [`Configurable::proxy_authentication`].
    #[must_use = "builders have no effect if unused"]
    fn proxy_credentials(self, credentials: Credentials) -> Self {
        self.with_config(move |config| {
            config.proxy_credentials = Some(Proxy(credentials));
        })
    }

    /// Set a maximum upload speed for the request body, in bytes per second.
    ///
    /// The default is unlimited.
    #[must_use = "builders have no effect if unused"]
    fn max_upload_speed(self, max: u64) -> Self {
        self.with_config(move |config| {
            config.max_upload_speed = Some(max);
        })
    }

    /// Set a maximum download speed for the response body, in bytes per second.
    ///
    /// The default is unlimited.
    #[must_use = "builders have no effect if unused"]
    fn max_download_speed(self, max: u64) -> Self {
        self.with_config(move |config| {
            config.max_download_speed = Some(max);
        })
    }

    /// Set a custom SSL/TLS client certificate to use for client connections.
    ///
    /// If a format is not supported by the underlying SSL/TLS engine, an error
    /// will be returned when attempting to send a request using the offending
    /// certificate.
    ///
    /// The default value is none.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::{
    ///     config::{ClientCertificate, PrivateKey},
    ///     prelude::*,
    ///     Request,
    /// };
    ///
    /// let response = Request::get("localhost:3999")
    ///     .ssl_client_certificate(ClientCertificate::pem_file(
    ///         "client.pem",
    ///         PrivateKey::pem_file("key.pem", String::from("secret")),
    ///     ))
    ///     .body(())?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    ///
    /// ```
    /// use isahc::{
    ///     config::{ClientCertificate, PrivateKey},
    ///     prelude::*,
    ///     HttpClient,
    /// };
    ///
    /// let client = HttpClient::builder()
    ///     .ssl_client_certificate(ClientCertificate::pem_file(
    ///         "client.pem",
    ///         PrivateKey::pem_file("key.pem", String::from("secret")),
    ///     ))
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn ssl_client_certificate(self, certificate: ClientCertificate) -> Self {
        self.with_config(move |config| {
            config.ssl_client_certificate = Some(certificate);
        })
    }

    /// Set a custom SSL/TLS CA certificate bundle to use for client
    /// connections.
    ///
    /// The default value is none.
    ///
    /// # Notes
    ///
    /// On Windows it may be necessary to combine this with
    /// [`SslOption::DANGER_ACCEPT_REVOKED_CERTS`] in order to work depending on
    /// the contents of your CA bundle.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::{config::CaCertificate, prelude::*, HttpClient};
    ///
    /// let client = HttpClient::builder()
    ///     .ssl_ca_certificate(CaCertificate::file("ca.pem"))
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn ssl_ca_certificate(self, certificate: CaCertificate) -> Self {
        self.with_config(move |config| {
            config.ssl_ca_certificate = Some(certificate);
        })
    }

    /// Set a list of ciphers to use for SSL/TLS connections.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// engine in use. You can find an up-to-date list of potential cipher names
    /// at <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in the system defaults being used.
    #[must_use = "builders have no effect if unused"]
    fn ssl_ciphers<I, T>(self, ciphers: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.with_config(move |config| {
            config.ssl_ciphers = Some(ciphers.into_iter().map(T::into).collect());
        })
    }

    /// Set various options for this request that control SSL/TLS behavior.
    ///
    /// Most options are for disabling security checks that introduce security
    /// risks, but may be required as a last resort. Note that the most secure
    /// options are already the default and do not need to be specified.
    ///
    /// The default value is [`SslOption::NONE`].
    ///
    /// # Warning
    ///
    /// You should think very carefully before using this method. Using *any*
    /// options that alter how certificates are validated can introduce
    /// significant security vulnerabilities.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::{config::SslOption, prelude::*, Request};
    ///
    /// let response = Request::get("https://badssl.com")
    ///     .ssl_options(SslOption::DANGER_ACCEPT_INVALID_CERTS | SslOption::DANGER_ACCEPT_REVOKED_CERTS)
    ///     .body(())?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    ///
    /// ```
    /// use isahc::{config::SslOption, prelude::*, HttpClient};
    ///
    /// let client = HttpClient::builder()
    ///     .ssl_options(SslOption::DANGER_ACCEPT_INVALID_CERTS | SslOption::DANGER_ACCEPT_REVOKED_CERTS)
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[must_use = "builders have no effect if unused"]
    fn ssl_options(self, options: SslOption) -> Self {
        self.with_config(move |config| {
            config.ssl_options = Some(options);
        })
    }

    /// Enable or disable sending HTTP header names in Title-Case instead of
    /// lowercase form.
    ///
    /// This option only affects user-supplied headers and does not affect
    /// low-level headers that are automatically supplied for HTTP protocol
    /// details, such as `Connection` and `Host` (unless you override such a
    /// header yourself).
    ///
    /// This option has no effect when using HTTP/2 or newer where headers are
    /// required to be lowercase.
    #[must_use = "builders have no effect if unused"]
    fn title_case_headers(self, enable: bool) -> Self {
        self.with_config(move |config| {
            config.title_case_headers = Some(enable);
        })
    }

    /// Enable or disable comprehensive per-request metrics collection.
    ///
    /// When enabled, detailed timing metrics will be tracked while a request is
    /// in progress, such as bytes sent and received, estimated size, DNS lookup
    /// time, etc. For a complete list of the available metrics that can be
    /// inspected, see the [`Metrics`](crate::Metrics) documentation.
    ///
    /// When enabled, to access a view of the current metrics values you can use
    /// [`ResponseExt::metrics`](crate::ResponseExt::metrics).
    ///
    /// While effort is taken to optimize hot code in metrics collection, it is
    /// likely that enabling it will have a small effect on overall throughput.
    /// Disabling metrics may be necessary for absolute peak performance.
    ///
    /// By default metrics are disabled.
    #[must_use = "builders have no effect if unused"]
    fn metrics(self, enable: bool) -> Self {
        self.with_config(move |config| {
            config.enable_metrics = Some(enable);
        })
    }
}

/// A strategy for selecting what HTTP versions should be used when
/// communicating with a server.
///
/// You can set a version negotiation strategy on a given request or on a client
/// with [`Configurable::version_negotiation`].
///
/// Attempting to use an HTTP version without client-side support at runtime
/// will result in an error. For example, using the system libcurl on an old
/// machine may not have an HTTP/2 implementation. Using static linking and the
/// [`http2`](../index.html#http2) crate feature can help guarantee that HTTP/2
/// will be available to use.
#[derive(Clone, Debug)]
pub struct VersionNegotiation(VersionNegotiationInner);

#[derive(Clone, Copy, Debug)]
enum VersionNegotiationInner {
    LatestCompatible,
    Strict(curl::easy::HttpVersion),
}

impl Default for VersionNegotiation {
    fn default() -> Self {
        Self::latest_compatible()
    }
}

impl VersionNegotiation {
    /// Always prefer the latest supported version announced by the server,
    /// falling back to older versions if not explicitly listed as supported.
    /// This is the default.
    ///
    /// Secure connections will begin with a TLS handshake, after which the
    /// highest supported HTTP version listed by the server via ALPN will be
    /// used. Once connected, additional upgrades to newer versions may also
    /// occur if the server lists support for it. In the future, headers such as
    /// `Alt-Svc` will be used.
    ///
    /// Insecure connections always use HTTP/1.x since there is no standard
    /// mechanism for a server to declare support for insecure HTTP versions,
    /// and only HTTP/0.9, HTTP/1.x, and HTTP/2 support insecure transfers.
    pub const fn latest_compatible() -> Self {
        Self(VersionNegotiationInner::LatestCompatible)
    }

    /// Connect via HTTP/1.0 and do not attempt to use a higher version.
    pub const fn http10() -> Self {
        Self(VersionNegotiationInner::Strict(
            curl::easy::HttpVersion::V10,
        ))
    }

    /// Connect via HTTP/1.1 and do not attempt to use a higher version.
    pub const fn http11() -> Self {
        Self(VersionNegotiationInner::Strict(
            curl::easy::HttpVersion::V11,
        ))
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
        Self(VersionNegotiationInner::Strict(
            curl::easy::HttpVersion::V2PriorKnowledge,
        ))
    }

    /// Connect via HTTP/3. Failure to connect will not fall back to old
    /// versions.
    pub const fn http3() -> Self {
        Self(VersionNegotiationInner::Strict(curl::easy::HttpVersion::V3))
    }
}

impl SetOpt for VersionNegotiation {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match self.0 {
            VersionNegotiationInner::LatestCompatible => {
                // If HTTP/2 support is available, this basically the most
                // lenient way of using it. Alt-Svc is used to upgrade to newer
                // versions, and old versions are used if the server doesn't
                // list HTTP/2 via ALPN.
                //
                // If HTTP/2 is not available, leaving it the default setting is
                // the ideal behavior.
                if is_http_version_supported(http::Version::HTTP_2) {
                    easy.http_version(curl::easy::HttpVersion::V2TLS)
                } else {
                    Ok(())
                }
            }
            VersionNegotiationInner::Strict(version) => easy.http_version(version),
        }
    }
}

/// Used to configure which local addresses or interfaces should be used to send
/// network traffic from.
///
/// Note that this type is "lazy" in the sense that errors are not returned if
/// the given interfaces are not checked for validity until you actually attempt
/// to use it in a network request.
#[derive(Clone, Debug)]
pub struct NetworkInterface {
    /// Interface in verbose curl format.
    interface: Option<String>,
}

impl NetworkInterface {
    /// Bind to whatever the networking stack finds suitable. This is the
    /// default behavior.
    pub fn any() -> Self {
        Self {
            interface: None,
        }
    }

    /// Bind to the interface with the given name (such as `eth0`). This method
    /// is not available on Windows as it does not really have names for network
    /// devices.
    ///
    /// # Examples
    ///
    /// ```
    /// # use isahc::config::NetworkInterface;
    /// let loopback = NetworkInterface::name("lo");
    /// let wifi = NetworkInterface::name("wlan0");
    /// ```
    #[cfg(unix)]
    pub fn name(name: impl AsRef<str>) -> Self {
        Self {
            interface: Some(format!("if!{}", name.as_ref())),
        }
    }

    /// Bind to the given local host or address. This can either be a host name
    /// or an IP address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use isahc::config::NetworkInterface;
    /// let local = NetworkInterface::host("server.local");
    /// let addr = NetworkInterface::host("192.168.1.2");
    /// ```
    pub fn host(host: impl AsRef<str>) -> Self {
        Self {
            interface: Some(format!("host!{}", host.as_ref())),
        }
    }
}

impl Default for NetworkInterface {
    fn default() -> Self {
        Self::any()
    }
}

impl From<IpAddr> for NetworkInterface {
    fn from(ip: IpAddr) -> Self {
        Self {
            interface: Some(format!("host!{}", ip)),
        }
    }
}

impl SetOpt for NetworkInterface {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        #[allow(unsafe_code)]
        match self.interface.as_ref() {
            Some(interface) => easy.interface(interface),

            // Use raw FFI because safe wrapper doesn't let us set to null.
            None => unsafe {
                match curl_sys::curl_easy_setopt(easy.raw(), curl_sys::CURLOPT_INTERFACE, 0) {
                    curl_sys::CURLE_OK => Ok(()),
                    code => Err(curl::Error::new(code)),
                }
            },
        }
    }
}

/// Supported IP versions that can be used.
#[derive(Clone, Debug)]
pub enum IpVersion {
    /// Use IPv4 addresses only. IPv6 addresses will be ignored.
    V4,

    /// Use IPv6 addresses only. IPv4 addresses will be ignored.
    V6,

    /// Use either IPv4 or IPv6 addresses. By default IPv6 addresses are
    /// preferred if available, otherwise an IPv4 address will be used. IPv6
    /// addresses are tried first by following the recommendations of [RFC
    /// 6555 "Happy Eyeballs"](https://tools.ietf.org/html/rfc6555).
    Any,
}

impl Default for IpVersion {
    fn default() -> Self {
        Self::Any
    }
}

impl SetOpt for IpVersion {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.ip_resolve(match &self {
            IpVersion::V4 => curl::easy::IpResolve::V4,
            IpVersion::V6 => curl::easy::IpResolve::V6,
            IpVersion::Any => curl::easy::IpResolve::Any,
        })
    }
}

/// Controls the use of the `Expect` request header when sending request bodies
/// with HTTP/1.1.
///
/// By default, when sending requests containing a body of large or unknown
/// length over HTTP/1.1, Isahc will send the request headers first without the
/// body and wait for the server to respond with a 100 (Continue) status code,
/// as defined by [RFC 7231, Section
/// 5.1.1](https://datatracker.ietf.org/doc/html/rfc7231#section-5.1.1). This
/// gives the opportunity for the server to reject the response without needing
/// to first transmit the request body over the network, if the body contents
/// are not necessary for the server to determine an appropriate response.
///
/// For servers that do not support this behavior and instead simply wait for
/// the request body without responding with a 100 (Continue), there is a
/// limited timeout before the response body is sent anyway without
/// confirmation. The default timeout is 1 second, but this can be configured.
///
/// The `Expect` behavior can also be disabled entirely.
///
/// This configuration only takes effect when using HTTP/1.1.
#[derive(Clone, Debug)]
pub struct ExpectContinue {
    timeout: Option<Duration>,
}

impl ExpectContinue {
    /// Enable the use of `Expect` and wait for a 100 (Continue) response with a
    /// default timeout before sending a request body.
    pub const fn enabled() -> Self {
        Self::timeout(Duration::from_secs(1))
    }

    /// Enable the use of `Expect` and wait for a 100 (Continue) response, up to
    /// the given timeout, before sending a request body.
    pub const fn timeout(timeout: Duration) -> Self {
        Self {
            timeout: Some(timeout),
        }
    }

    /// Disable the use and handling of the `Expect` request header.
    pub const fn disabled() -> Self {
        Self {
            timeout: None,
        }
    }

    pub(crate) fn is_disabled(&self) -> bool {
        self.timeout.is_none()
    }
}

impl Default for ExpectContinue {
    fn default() -> Self {
        Self::enabled()
    }
}

impl From<bool> for ExpectContinue {
    fn from(value: bool) -> Self {
        if value {
            Self::enabled()
        } else {
            Self::disabled()
        }
    }
}

impl From<Duration> for ExpectContinue {
    fn from(value: Duration) -> Self {
        Self::timeout(value)
    }
}

impl SetOpt for ExpectContinue {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        if let Some(timeout) = self.timeout {
            easy.expect_100_timeout(timeout)
        } else {
            Ok(())
        }
    }
}
