use crate::{
    auth::{Authentication, Credentials},
    client::ResponseFuture,
    config::*,
    {Body, Error},
};
use http::{Request, Response};
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Duration;

/// Provides additional methods when building a request for configuring various
/// execution-related options on how the request should be sent.
pub trait RequestBuilderExt {
    /// Set a maximum amount of time that the request is allowed to take before
    /// being aborted.
    ///
    /// If not set, no timeout will be enforced.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
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
    fn timeout(&mut self, timeout: Duration) -> &mut Self;

    /// Set a timeout for the initial connection phase.
    ///
    /// If not set, a connect timeout of 300 seconds will be used.
    fn connect_timeout(&mut self, timeout: Duration) -> &mut Self;

    /// Configure how the use of HTTP versions should be negotiated with the
    /// server.
    ///
    /// The default is [`HttpVersionNegotiation::latest_compatible`].
    fn version_negotiation(&mut self, negotiation: VersionNegotiation) -> &mut Self;

    /// Set a policy for automatically following server redirects.
    ///
    /// The default is to not follow redirects.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::config::RedirectPolicy;
    /// use isahc::prelude::*;
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
    fn redirect_policy(&mut self, policy: RedirectPolicy) -> &mut Self;

    /// Update the `Referer` header automatically when following redirects.
    fn auto_referer(&mut self) -> &mut Self;

    /// Set one or more HTTP authentication methods to attempt to use when
    /// authenticating with the server.
    ///
    /// Depending on the authentication schemes enabled, you will also need to
    /// set credentials to use for authentication using
    /// [`RequestBuilderExt::credentials`].
    fn authentication(&mut self, authentication: Authentication) -> &mut Self;

    /// Set the credentials to use for HTTP authentication on this requests.
    ///
    /// This setting will do nothing unless you also set one or more
    /// authentication methods using [`RequestBuilderExt::authentication`].
    fn credentials(&mut self, credentials: Credentials) -> &mut Self;

    /// Enable TCP keepalive with a given probe interval.
    fn tcp_keepalive(&mut self, interval: Duration) -> &mut Self;

    /// Enables the `TCP_NODELAY` option on connect.
    fn tcp_nodelay(&mut self) -> &mut Self;

    /// Set a proxy to use for the request.
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
    /// Setting to `None` explicitly disable the use of a proxy.
    fn proxy(&mut self, proxy: impl Into<Option<http::Uri>>) -> &mut Self;

    /// Disable proxy usage to use for the provided list of hosts.
    fn proxy_blacklist(&mut self, hosts: impl IntoIterator<Item = String>) -> &mut Self;

    /// Set one or more HTTP authentication methods to attempt to use when
    /// authenticating with a proxy.
    ///
    /// Depending on the authentication schemes enabled, you will also need to
    /// set credentials to use for authentication using
    /// [`RequestBuilderExt::proxy_credentials`].
    fn proxy_authentication(&mut self, authentication: Authentication) -> &mut Self;

    /// Set the credentials to use for proxy authentication.
    ///
    /// This setting will do nothing unless you also set one or more proxy
    /// authentication methods using
    /// [`RequestBuilderExt::proxy_authentication`].
    fn proxy_credentials(&mut self, credentials: Credentials) -> &mut Self;

    /// Set a maximum upload speed for the request body, in bytes per second.
    ///
    /// The default is unlimited.
    fn max_upload_speed(&mut self, max: u64) -> &mut Self;

    /// Set a maximum download speed for the response body, in bytes per second.
    ///
    /// The default is unlimited.
    fn max_download_speed(&mut self, max: u64) -> &mut Self;

    /// Set a list of specific DNS servers to be used for DNS resolution.
    ///
    /// By default this option is not set and the system's built-in DNS resolver
    /// is used. This option can only be used if libcurl is compiled with
    /// [c-ares](https://c-ares.haxx.se), otherwise this option has no effect.
    fn dns_servers(&mut self, servers: impl IntoIterator<Item = SocketAddr>) -> &mut Self;

    /// Set a custom SSL/TLS client certificate to use for all client
    /// connections.
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
    /// use isahc::config::{ClientCertificate, PrivateKey};
    /// use isahc::prelude::*;
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
    fn ssl_client_certificate(&mut self, certificate: ClientCertificate) -> &mut Self;

    /// Set a custom SSL/TLS CA certificate bundle to use.
    ///
    /// The default value is none.
    fn ssl_ca_certificate(&mut self, certificate: CaCertificate) -> &mut Self;

    /// Set a list of ciphers to use for SSL/TLS connections.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// engine in use. You can find an up-to-date list of potential cipher names
    /// at <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in the system defaults being used.
    fn ssl_ciphers(&mut self, servers: impl IntoIterator<Item = String>) -> &mut Self;

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
    /// ```
    /// # use isahc::config::*;
    /// # use isahc::prelude::*;
    /// #
    /// let response = Request::get("https://badssl.com")
    ///     .ssl_options(SslOption::DANGER_ACCEPT_INVALID_CERTS | SslOption::DANGER_ACCEPT_REVOKED_CERTS)
    ///     .body(())?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn ssl_options(&mut self, options: SslOption) -> &mut Self;

    /// Enable or disable comprehensive metrics collection for this request.
    ///
    /// See [`HttpClientBuilder::metrics`](crate::HttpClientBuilder::metrics)
    /// for details.
    fn metrics(&mut self, enable: bool) -> &mut Self;
}

impl RequestBuilderExt for http::request::Builder {
    fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.extension(Timeout(timeout))
    }

    fn connect_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.extension(ConnectTimeout(timeout))
    }

    fn version_negotiation(&mut self, negotiation: VersionNegotiation) -> &mut Self {
        self.extension(negotiation)
    }

    fn redirect_policy(&mut self, policy: RedirectPolicy) -> &mut Self {
        self.extension(policy)
    }

    fn auto_referer(&mut self) -> &mut Self {
        self.extension(AutoReferer)
    }

    fn authentication(&mut self, authentication: Authentication) -> &mut Self {
        self.extension(authentication)
    }

    fn credentials(&mut self, credentials: Credentials) -> &mut Self {
        self.extension(credentials)
    }

    fn tcp_keepalive(&mut self, interval: Duration) -> &mut Self {
        self.extension(TcpKeepAlive(interval))
    }

    fn tcp_nodelay(&mut self) -> &mut Self {
        self.extension(TcpNoDelay)
    }

    fn proxy(&mut self, proxy: impl Into<Option<http::Uri>>) -> &mut Self {
        self.extension(Proxy(proxy.into()))
    }

    fn proxy_blacklist(&mut self, hosts: impl IntoIterator<Item = String>) -> &mut Self {
        self.extension(ProxyBlacklist::from_iter(hosts))
    }

    fn proxy_authentication(&mut self, authentication: Authentication) -> &mut Self {
        self.extension(Proxy(authentication))
    }

    fn proxy_credentials(&mut self, credentials: Credentials) -> &mut Self {
        self.extension(Proxy(credentials))
    }

    fn max_upload_speed(&mut self, max: u64) -> &mut Self {
        self.extension(MaxUploadSpeed(max))
    }

    fn max_download_speed(&mut self, max: u64) -> &mut Self {
        self.extension(MaxDownloadSpeed(max))
    }

    fn dns_servers(&mut self, servers: impl IntoIterator<Item = SocketAddr>) -> &mut Self {
        self.extension(DnsServers::from_iter(servers))
    }

    fn ssl_ciphers(&mut self, servers: impl IntoIterator<Item = String>) -> &mut Self {
        self.extension(ssl::Ciphers::from_iter(servers))
    }

    fn ssl_client_certificate(&mut self, certificate: ClientCertificate) -> &mut Self {
        self.extension(certificate)
    }

    fn ssl_ca_certificate(&mut self, certificate: CaCertificate) -> &mut Self {
        self.extension(certificate)
    }

    fn ssl_options(&mut self, options: SslOption) -> &mut Self {
        self.extension(options)
    }

    fn metrics(&mut self, enable: bool) -> &mut Self {
        self.extension(EnableMetrics(enable))
    }
}

/// Extension methods on an HTTP request.
pub trait RequestExt<T> {
    /// Send the HTTP request synchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send`](crate::send).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let response = Request::post("https://httpbin.org/post")
    ///     .header("Content-Type", "application/json")
    ///     .body(r#"{
    ///         "speed": "fast",
    ///         "cool_name": true
    ///     }"#)?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>;

    /// Sends the HTTP request asynchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send_async`](crate::send_async).
    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<Body>;
}

impl<T> RequestExt<T> for Request<T> {
    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>,
    {
        crate::send(self)
    }

    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<Body>,
    {
        crate::send_async(self)
    }
}
