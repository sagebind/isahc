//! The HTTP client implementation.

use crate::agent;
use crate::config::*;
use crate::handler::{RequestHandler, RequestHandlerFuture};
use crate::middleware::Middleware;
use crate::{Body, Error};
use http::{Request, Response};
use lazy_static::lazy_static;
use std::fmt;
use std::future::Future;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

lazy_static! {
    static ref USER_AGENT: String = format!(
        "curl/{} isahc/{}",
        curl::Version::get().version(),
        env!("CARGO_PKG_VERSION")
    );
}

/// An HTTP client builder, capable of creating custom [`HttpClient`] instances
/// with customized behavior.
///
/// # Examples
///
/// ```
/// use isahc::config::RedirectPolicy;
/// use isahc::http;
/// use isahc::prelude::*;
/// use std::time::Duration;
///
/// let client = HttpClient::builder()
///     .timeout(Duration::from_secs(60))
///     .redirect_policy(RedirectPolicy::Limit(10))
///     .preferred_http_version(http::Version::HTTP_2)
///     .build()?;
/// # Ok::<(), isahc::Error>(())
/// ```
#[derive(Default)]
pub struct HttpClientBuilder {
    defaults: http::Extensions,
    middleware: Vec<Box<dyn Middleware>>,
}

impl HttpClientBuilder {
    /// Create a new builder for building a custom client. All configuration
    /// will start out with the default values.
    ///
    /// This is equivalent to the [`Default`] implementation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable persistent cookie handling using a cookie jar.
    #[cfg(feature = "cookies")]
    pub fn cookies(self) -> Self {
        self.middleware_impl(crate::cookies::CookieJar::default())
    }

    /// Add a middleware layer to the client.
    #[cfg(feature = "middleware-api")]
    pub fn middleware(self, middleware: impl Middleware) -> Self {
        self.middleware_impl(middleware)
    }

    #[allow(unused)]
    fn middleware_impl(mut self, middleware: impl Middleware) -> Self {
        self.middleware.push(Box::new(middleware));
        self
    }

    /// Set a timeout for the maximum time allowed for a request-response cycle.
    ///
    /// If not set, no timeout will be enforced.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.defaults.insert(Timeout(timeout));
        self
    }

    /// Set a timeout for the initial connection phase.
    ///
    /// If not set, a connect timeout of 300 seconds will be used.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.defaults.insert(ConnectTimeout(timeout));
        self
    }

    /// Set a policy for automatically following server redirects.
    ///
    /// The default is to not follow redirects.
    pub fn redirect_policy(mut self, policy: RedirectPolicy) -> Self {
        self.defaults.insert(policy);
        self
    }

    /// Update the `Referer` header automatically when following redirects.
    pub fn auto_referer(mut self) -> Self {
        self.defaults.insert(AutoReferer);
        self
    }

    /// Set a preferred HTTP version the client should attempt to use to
    /// communicate to the server with.
    ///
    /// This is treated as a suggestion. A different version may be used if the
    /// server does not support it or negotiates a different version.
    pub fn preferred_http_version(mut self, version: http::Version) -> Self {
        self.defaults.insert(PreferredHttpVersion(version));
        self
    }

    /// Enable TCP keepalive with a given probe interval.
    pub fn tcp_keepalive(mut self, interval: Duration) -> Self {
        self.defaults.insert(TcpKeepAlive(interval));
        self
    }

    /// Enables the `TCP_NODELAY` option on connect.
    pub fn tcp_nodelay(mut self) -> Self {
        self.defaults.insert(TcpNoDelay);
        self
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
    pub fn proxy(mut self, proxy: http::Uri) -> Self {
        self.defaults.insert(Proxy(proxy));
        self
    }

    /// Set a maximum upload speed for the request body, in bytes per second.
    ///
    /// The default is unlimited.
    pub fn max_upload_speed(mut self, max: u64) -> Self {
        self.defaults.insert(MaxUploadSpeed(max));
        self
    }

    /// Set a maximum download speed for the response body, in bytes per second.
    ///
    /// The default is unlimited.
    pub fn max_download_speed(mut self, max: u64) -> Self {
        self.defaults.insert(MaxDownloadSpeed(max));
        self
    }

    /// Set a list of specific DNS servers to be used for DNS resolution.
    ///
    /// By default this option is not set and the system's built-in DNS resolver
    /// is used. This option can only be used if libcurl is compiled with
    /// [c-ares](https://c-ares.haxx.se), otherwise this option has no effect.
    pub fn dns_servers(mut self, servers: impl IntoIterator<Item = SocketAddr>) -> Self {
        self.defaults.insert(DnsServers::from_iter(servers));
        self
    }

    /// Set a list of ciphers to use for SSL/TLS connections.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// engine in use. You can find an up-to-date list of potential cipher names
    /// at <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in the system defaults being used.
    pub fn ssl_ciphers(mut self, servers: impl IntoIterator<Item = String>) -> Self {
        self.defaults.insert(SslCiphers::from_iter(servers));
        self
    }

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
    /// ```
    /// # use isahc::config::*;
    /// # use isahc::prelude::*;
    /// #
    /// let client = HttpClient::builder()
    ///     .ssl_client_certificate(ClientCertificate::PEM {
    ///         path: "client.pem".into(),
    ///         private_key: Some(PrivateKey::PEM {
    ///             path: "key.pem".into(),
    ///             password: Some("secret".into()),
    ///         }),
    ///     })
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    pub fn ssl_client_certificate(mut self, certificate: ClientCertificate) -> Self {
        self.defaults.insert(certificate);
        self
    }

    /// Build an [`HttpClient`] using the configured options.
    ///
    /// If the client fails to initialize, an error will be returned.
    pub fn build(self) -> Result<HttpClient, Error> {
        Ok(HttpClient {
            agent: agent::new()?,
            defaults: self.defaults,
            middleware: self.middleware,
        })
    }
}

impl fmt::Debug for HttpClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClientBuilder").finish()
    }
}

/// An HTTP client for making requests.
///
/// An [`HttpClient`] instance acts as a session for executing one or more HTTP
/// requests, and also allows you to set common protocol settings that should be
/// applied to all requests made with the client.
///
/// [`HttpClient`] is entirely thread-safe, and implements both [`Send`] and
/// [`Sync`]. You are free to create clients outside the context of the "main"
/// thread, or move them between threads. You can even invoke many requests
/// simultaneously from multiple threads, since doing so doesn't need a mutable
/// reference to the client. This is fairly cheap to do as well, since
/// internally requests use lock-free message passing to get things going.
///
/// The client maintains a connection pool internally and is not cheap to
/// create, so we recommend creating a client once and re-using it throughout
/// your code. Creating a new client for every request would decrease
/// performance significantly, and might cause errors to occur under high
/// workloads, caused by creating too many system resources like sockets or
/// threads.
///
/// It is not universally true that you should use exactly one client instance
/// in an application. All HTTP requests made with the same client will share
/// any session-wide state, like cookies or persistent connections. It may be
/// the case that it is better to create separate clients for separate areas of
/// an application if they have separate concerns or are making calls to
/// different servers. If you are creating an API client library, that might be
/// a good place to maintain your own internal client.
///
/// # Examples
///
/// ```no_run
/// use isahc::prelude::*;
///
/// // Create a new client using reasonable defaults.
/// let client = HttpClient::default();
///
/// // Make some requests.
/// let mut response = client.get("https://example.org")?;
/// assert!(response.status().is_success());
///
/// println!("Response:\n{}", response.text()?);
/// # Ok::<(), isahc::Error>(())
/// ```
///
/// Customizing the client configuration:
///
/// ```no_run
/// use isahc::{config::RedirectPolicy, prelude::*};
/// use std::time::Duration;
///
/// let client = HttpClient::builder()
///     .preferred_http_version(http::Version::HTTP_11)
///     .redirect_policy(RedirectPolicy::Limit(10))
///     .timeout(Duration::from_secs(20))
///     // May return an error if there's something wrong with our configuration
///     // or if the client failed to start up.
///     .build()?;
///
/// let response = client.get("https://example.org")?;
/// assert!(response.status().is_success());
/// # Ok::<(), isahc::Error>(())
/// ```
///
/// See the documentation on [`HttpClientBuilder`] for a comprehensive look at
/// what can be configured.
pub struct HttpClient {
    /// This is how we talk to our background agent thread.
    agent: agent::Handle,
    /// Map of config values that should be used to configure execution if not
    /// specified in a request.
    defaults: http::Extensions,
    /// Any middleware implementations that requests should pass through.
    middleware: Vec<Box<dyn Middleware>>,
}

impl Default for HttpClient {
    fn default() -> Self {
        HttpClientBuilder::default()
            .build()
            .expect("client failed to initialize")
    }
}

impl HttpClient {
    /// Create a new HTTP client using the default configuration.
    ///
    /// This is equivalent to the [`Default`] implementation.
    ///
    /// # Panics
    ///
    /// Panics if any required internal systems fail to initialize. This might
    /// occur if creating a socket fails, spawning a thread fails, or if
    /// something else goes wrong.
    ///
    /// Generally such a panic indicates an internal bug or an issue with system
    /// configuration. If you need to catch these errors, you can use
    /// [`HttpClientBuilder::build`] instead.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a reference to a global client instance.
    ///
    /// TODO: Stabilize.
    pub(crate) fn shared() -> &'static Self {
        lazy_static! {
            static ref SHARED: HttpClient = HttpClient::new();
        }
        &SHARED
    }

    /// Create a new [`HttpClientBuilder`] for building a custom client.
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder::default()
    }

    /// Send a GET request to the given URI.
    ///
    /// To customize the request further, see [`HttpClient::send`]. To execute
    /// the request asynchronously, see [`HttpClient::get_async`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// # let client = HttpClient::default();
    /// let mut response = client.get("https://example.org")?;
    /// println!("{}", response.text()?);
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[inline]
    pub fn get<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.get_async(uri).join()
    }

    /// Send a GET request to the given URI asynchronously.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::get`].
    pub fn get_async<U>(&self, uri: U) -> ResponseFuture<'_>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.send_builder_async(http::Request::get(uri), Body::empty())
    }

    /// Send a HEAD request to the given URI.
    ///
    /// To customize the request further, see [`HttpClient::send`]. To execute
    /// the request asynchronously, see [`HttpClient::head_async`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use isahc::prelude::*;
    /// # let client = HttpClient::default();
    /// let response = client.head("https://example.org")?;
    /// println!("Page size: {:?}", response.headers()["content-length"]);
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[inline]
    pub fn head<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.head_async(uri).join()
    }

    /// Send a HEAD request to the given URI asynchronously.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::head`].
    pub fn head_async<U>(&self, uri: U) -> ResponseFuture<'_>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.send_builder_async(http::Request::head(uri), Body::empty())
    }

    /// Send a POST request to the given URI with a given request body.
    ///
    /// To customize the request further, see [`HttpClient::send`]. To execute
    /// the request asynchronously, see [`HttpClient::post_async`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let client = HttpClient::default();
    ///
    /// let response = client.post("https://httpbin.org/post", r#"{
    ///     "speed": "fast",
    ///     "cool_name": true
    /// }"#)?;
    /// # Ok::<(), isahc::Error>(())
    #[inline]
    pub fn post<U>(&self, uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.post_async(uri, body).join()
    }

    /// Send a POST request to the given URI asynchronously with a given request
    /// body.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::post`].
    pub fn post_async<U>(&self, uri: U, body: impl Into<Body>) -> ResponseFuture<'_>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.send_builder_async(http::Request::post(uri), body)
    }

    /// Send a PUT request to the given URI with a given request body.
    ///
    /// To customize the request further, see [`HttpClient::send`]. To execute
    /// the request asynchronously, see [`HttpClient::put_async`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let client = HttpClient::default();
    ///
    /// let response = client.put("https://httpbin.org/put", r#"{
    ///     "speed": "fast",
    ///     "cool_name": true
    /// }"#)?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[inline]
    pub fn put<U>(&self, uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.put_async(uri, body).join()
    }

    /// Send a PUT request to the given URI asynchronously with a given request
    /// body.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::put`].
    pub fn put_async<U>(&self, uri: U, body: impl Into<Body>) -> ResponseFuture<'_>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.send_builder_async(http::Request::put(uri), body)
    }

    /// Send a DELETE request to the given URI.
    ///
    /// To customize the request further, see [`HttpClient::send`]. To execute
    /// the request asynchronously, see [`HttpClient::delete_async`].
    #[inline]
    pub fn delete<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.delete_async(uri).join()
    }

    /// Send a DELETE request to the given URI asynchronously.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::delete`].
    pub fn delete_async<U>(&self, uri: U) -> ResponseFuture<'_>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        self.send_builder_async(http::Request::delete(uri), Body::empty())
    }

    /// Send an HTTP request and return the HTTP response.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    ///
    /// This client's configuration can be overridden for this request by
    /// configuring the request using methods provided by the
    /// [`RequestBuilderExt`](crate::prelude::RequestBuilderExt) trait.
    ///
    /// Upon success, will return a [`Response`] containing the status code,
    /// response headers, and response body from the server. The [`Response`] is
    /// returned as soon as the HTTP response headers are received; the
    /// connection will remain open to stream the response body in real time.
    /// Dropping the response body without fully consume it will close the
    /// connection early without downloading the rest of the response body.
    ///
    /// _Note that the actual underlying socket connection isn't necessarily
    /// closed on drop. It may remain open to be reused if pipelining is being
    /// used, the connection is configured as `keep-alive`, and so on._
    ///
    /// Since the response body is streamed from the server, it may only be
    /// consumed once. If you need to inspect the response body more than once,
    /// you will have to either read it into memory or write it to a file.
    ///
    /// To execute the request asynchronously, see [`HttpClient::send_async`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let client = HttpClient::default();
    ///
    /// let request = Request::post("https://httpbin.org/post")
    ///     .header("Content-Type", "application/json")
    ///     .body(r#"{
    ///         "speed": "fast",
    ///         "cool_name": true
    ///     }"#)?;
    ///
    /// let response = client.send(request)?;
    /// assert!(response.status().is_success());
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[inline]
    pub fn send<B: Into<Body>>(&self, request: Request<B>) -> Result<Response<Body>, Error> {
        self.send_async(request).join()
    }

    /// Send an HTTP request and return the HTTP response asynchronously.
    ///
    /// See [`HttpClient::send`] for further details.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use isahc::prelude::*;
    ///
    /// let client = HttpClient::default();
    ///
    /// let request = Request::post("https://httpbin.org/post")
    ///     .header("Content-Type", "application/json")
    ///     .body(r#"{
    ///         "speed": "fast",
    ///         "cool_name": true
    ///     }"#)?;
    ///
    /// let response = client.send_async(request).await?;
    /// assert!(response.status().is_success());
    /// ```
    pub fn send_async<B: Into<Body>>(&self, request: Request<B>) -> ResponseFuture<'_> {
        let mut request = request.map(Into::into);

        // Set default user agent if not specified.
        request
            .headers_mut()
            .entry(http::header::USER_AGENT)
            .unwrap()
            .or_insert(USER_AGENT.parse().unwrap());

        // Apply any request middleware, starting with the outermost one.
        for middleware in self.middleware.iter().rev() {
            request = middleware.filter_request(request);
        }

        ResponseFuture {
            client: self,
            error: None,
            request: Some(request),
            inner: None,
        }
    }

    fn send_builder_async(
        &self,
        mut builder: http::request::Builder,
        body: impl Into<Body>,
    ) -> ResponseFuture<'_> {
        match builder.body(body.into()) {
            Ok(request) => self.send_async(request),
            Err(e) => ResponseFuture {
                client: self,
                error: Some(e.into()),
                request: None,
                inner: None,
            },
        }
    }

    fn create_easy_handle(
        &self,
        request: Request<Body>,
    ) -> Result<(curl::easy::Easy2<RequestHandler>, RequestHandlerFuture), Error> {
        // Prepare the request plumbing.
        let (mut parts, body) = request.into_parts();
        let has_body = !body.is_empty();
        let body_length = body.len();
        let (handler, future) = RequestHandler::new(body);

        // Helper for fetching an extension first from the request, then falling
        // back to client defaults.
        macro_rules! extension {
            ($first:expr) => {
                $first.get()
            };

            ($first:expr, $($rest:expr),+) => {
                $first.get().or_else(|| extension!($($rest),*))
            };
        }

        let mut easy = curl::easy::Easy2::new(handler);

        easy.verbose(log::log_enabled!(log::Level::Trace))?;
        easy.signal(false)?;

        if let Some(Timeout(timeout)) = extension!(parts.extensions, self.defaults) {
            easy.timeout(*timeout)?;
        }

        if let Some(ConnectTimeout(timeout)) = extension!(parts.extensions, self.defaults) {
            easy.connect_timeout(*timeout)?;
        }

        if let Some(TcpKeepAlive(interval)) = extension!(parts.extensions, self.defaults) {
            easy.tcp_keepalive(true)?;
            easy.tcp_keepintvl(*interval)?;
        }

        if let Some(TcpNoDelay) = extension!(parts.extensions, self.defaults) {
            easy.tcp_nodelay(true)?;
        }

        if let Some(redirect_policy) = extension!(parts.extensions, self.defaults) {
            match redirect_policy {
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
        }

        if let Some(MaxUploadSpeed(limit)) = extension!(parts.extensions, self.defaults) {
            easy.max_send_speed(*limit)?;
        }

        if let Some(MaxDownloadSpeed(limit)) = extension!(parts.extensions, self.defaults) {
            easy.max_recv_speed(*limit)?;
        }

        // Set a preferred HTTP version to negotiate.
        easy.http_version(match extension!(parts.extensions, self.defaults) {
            Some(PreferredHttpVersion(http::Version::HTTP_10)) => curl::easy::HttpVersion::V10,
            Some(PreferredHttpVersion(http::Version::HTTP_11)) => curl::easy::HttpVersion::V11,
            Some(PreferredHttpVersion(http::Version::HTTP_2)) => curl::easy::HttpVersion::V2,
            _ => curl::easy::HttpVersion::Any,
        })?;

        if let Some(Proxy(proxy)) = extension!(parts.extensions, self.defaults) {
            easy.proxy(&format!("{}", proxy))?;
        }

        if let Some(DnsServers(addrs)) = extension!(parts.extensions, self.defaults) {
            let dns_string = addrs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            if let Err(e) = easy.dns_servers(&dns_string) {
                log::warn!("DNS servers could not be configured: {}", e);
            }
        }

        // Configure SSL options.
        if let Some(SslCiphers(ciphers)) = extension!(parts.extensions, self.defaults) {
            easy.ssl_cipher_list(&ciphers.join(":"))?;
        }

        if let Some(cert) = extension!(parts.extensions, self.defaults) {
            easy.ssl_client_certificate(cert)?;
        }

        // Enable automatic response decompression.
        easy.accept_encoding("")?;

        // Set the HTTP method to use. Curl ties in behavior with the request
        // method, so we need to configure this carefully.
        match (&parts.method, has_body) {
            // Normal GET request.
            (&http::Method::GET, false) => {
                easy.get(true)?;
            }
            // HEAD requests do not wait for a response payload.
            (&http::Method::HEAD, has_body) => {
                easy.upload(has_body)?;
                easy.nobody(true)?;
                easy.custom_request("HEAD")?;
            }
            // POST requests have special redirect behavior.
            (&http::Method::POST, _) => {
                easy.post(true)?;
            }
            // Normal PUT request.
            (&http::Method::PUT, _) => {
                easy.upload(true)?;
            }
            // Default case is to either treat request like a GET or PUT.
            (method, has_body) => {
                easy.upload(has_body)?;
                easy.custom_request(method.as_str())?;
            }
        }

        easy.url(&parts.uri.to_string())?;

        // If the request has a body, then we either need to tell curl how large
        // the body is if we know it, or tell curl to use chunked encoding. If
        // we do neither, curl will simply not send the body without warning.
        if has_body {
            // Use length given in Content-Length header, or the size defined by
            // the body itself.
            let body_length = parts.headers.get("Content-Length")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok())
                .or(body_length);

            if let Some(len) = body_length {
                if parts.method == http::Method::POST {
                    easy.post_field_size(len)?;
                } else {
                    easy.in_filesize(len)?;
                }
            } else {
                // Set the Transfer-Encoding header to instruct curl to use
                // chunked encoding. Replaces any existing values that may be
                // incorrect.
                parts.headers.insert(
                    "Transfer-Encoding",
                    http::header::HeaderValue::from_static("chunked"),
                );
            }
        }

        // Prepare header list to give to curl.
        let mut headers = curl::easy::List::new();
        for (name, value) in parts.headers.iter() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header)?;
        }
        easy.http_headers(headers)?;

        Ok((easy, future))
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient").finish()
    }
}

/// Helper extension methods for curl easy handles.
trait EasyExt {
    fn easy(&mut self) -> &mut curl::easy::Easy2<RequestHandler>;

    fn ssl_client_certificate(&mut self, cert: &ClientCertificate) -> Result<(), curl::Error> {
        match cert {
            ClientCertificate::PEM { path, private_key } => {
                self.easy().ssl_cert(path)?;
                self.easy().ssl_cert_type("PEM")?;
                if let Some(key) = private_key {
                    self.ssl_private_key(key)?;
                }
            }
            ClientCertificate::DER { path, private_key } => {
                self.easy().ssl_cert(path)?;
                self.easy().ssl_cert_type("DER")?;
                if let Some(key) = private_key {
                    self.ssl_private_key(key)?;
                }
            }
            ClientCertificate::P12 { path, password } => {
                self.easy().ssl_cert(path)?;
                self.easy().ssl_cert_type("P12")?;
                if let Some(password) = password {
                    self.easy().key_password(password)?;
                }
            }
        }

        Ok(())
    }

    fn ssl_private_key(&mut self, key: &PrivateKey) -> Result<(), curl::Error> {
        match key {
            PrivateKey::PEM { path, password } => {
                self.easy().ssl_key(path)?;
                self.easy().ssl_key_type("PEM")?;
                if let Some(password) = password {
                    self.easy().key_password(password)?;
                }
            }
            PrivateKey::DER { path, password } => {
                self.easy().ssl_key(path)?;
                self.easy().ssl_key_type("DER")?;
                if let Some(password) = password {
                    self.easy().key_password(password)?;
                }
            }
        }

        Ok(())
    }
}

impl EasyExt for curl::easy::Easy2<RequestHandler> {
    fn easy(&mut self) -> &mut Self {
        self
    }
}

/// A future for a request being executed.
#[derive(Debug)]
pub struct ResponseFuture<'c> {
    /// The client this future is associated with.
    client: &'c HttpClient,
    /// A pre-filled error to return.
    error: Option<Error>,
    /// The request to send.
    request: Option<Request<Body>>,
    /// The inner future for actual execution.
    inner: Option<RequestHandlerFuture>,
}

impl<'c> ResponseFuture<'c> {
    fn maybe_initialize(&mut self) -> Result<(), Error> {
        // If the future has a pre-filled error, return that.
        if let Some(e) = self.error.take() {
            return Err(e);
        }

        // Request has not been sent yet.
        if let Some(request) = self.request.take() {
            // Create and configure a curl easy handle to fulfil the request.
            let (easy, future) = self.client.create_easy_handle(request)?;

            // Send the request to the agent to be executed.
            self.client.agent.submit_request(easy)?;

            self.inner = Some(future);
        }

        Ok(())
    }

    fn complete(&self, output: <Self as Future>::Output) -> <Self as Future>::Output {
        output.map(|mut response| {
            // Apply response middleware, starting with the innermost
            // one.
            for middleware in self.client.middleware.iter() {
                response = middleware.filter_response(response);
            }

            response
        })
    }

    /// Block the current thread until the request is completed or aborted. This
    /// effectively turns the asynchronous request into a synchronous one.
    fn join(mut self) -> Result<Response<Body>, Error> {
        self.maybe_initialize()?;

        if let Some(inner) = self.inner.take() {
            self.complete(inner.join())
        } else {
            panic!("join called after poll");
        }
    }
}

impl Future for ResponseFuture<'_> {
    type Output = Result<Response<Body>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.maybe_initialize()?;

        if let Some(inner) = self.inner.as_mut() {
            Pin::new(inner).poll(cx).map(|result| self.complete(result))
        } else {
            // Invalid state (called poll() after ready), just return pending...
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>() {}
    fn is_sync<T: Sync>() {}

    #[test]
    fn traits() {
        is_send::<HttpClient>();
        is_sync::<HttpClient>();

        is_send::<HttpClientBuilder>();
    }
}
