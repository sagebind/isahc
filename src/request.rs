use crate::client::ResponseFuture;
use crate::config::*;
use crate::{Body, Error};
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

    /// Set a preferred HTTP version the client should attempt to use to
    /// communicate to the server with.
    ///
    /// This is treated as a suggestion. A different version may be used if the
    /// server does not support it or negotiates a different version.
    fn preferred_http_version(&mut self, version: http::Version) -> &mut Self;

    /// Enable TCP keepalive with a given probe interval.
    fn tcp_keepalive(&mut self, interval: Duration) -> &mut Self;

    /// Enables the `TCP_NODELAY` option on connect.
    fn tcp_nodelay(&mut self) -> &mut Self;

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
    fn proxy(&mut self, proxy: http::Uri) -> &mut Self;

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

    /// Set a list of ciphers to use for SSL/TLS connections.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// engine in use. You can find an up-to-date list of potential cipher names
    /// at <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in the system defaults being used.
    fn ssl_ciphers(&mut self, servers: impl IntoIterator<Item = String>) -> &mut Self;

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
    ///     .ssl_client_certificate(ClientCertificate::PEM {
    ///         path: "client.pem".into(),
    ///         private_key: Some(PrivateKey::PEM {
    ///             path: "key.pem".into(),
    ///             password: Some("secret".into()),
    ///         }),
    ///     })
    ///     .body(())?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn ssl_client_certificate(&mut self, certificate: ClientCertificate) -> &mut Self;
}

impl RequestBuilderExt for http::request::Builder {
    fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.extension(Timeout(timeout))
    }

    fn connect_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.extension(ConnectTimeout(timeout))
    }

    fn redirect_policy(&mut self, policy: RedirectPolicy) -> &mut Self {
        self.extension(policy)
    }

    fn auto_referer(&mut self) -> &mut Self {
        self.extension(AutoReferer)
    }

    fn preferred_http_version(&mut self, version: http::Version) -> &mut Self {
        self.extension(PreferredHttpVersion(version))
    }

    fn tcp_keepalive(&mut self, interval: Duration) -> &mut Self {
        self.extension(TcpKeepAlive(interval))
    }

    fn tcp_nodelay(&mut self) -> &mut Self {
        self.extension(TcpNoDelay)
    }

    fn proxy(&mut self, proxy: http::Uri) -> &mut Self {
        self.extension(Proxy(proxy))
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
        self.extension(SslCiphers::from_iter(servers))
    }

    fn ssl_client_certificate(&mut self, certificate: ClientCertificate) -> &mut Self {
        self.extension(certificate)
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
