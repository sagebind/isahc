//! The HTTP client implementation.

use crate::{
    agent::{self, AgentBuilder},
    auth::{Authentication, Credentials},
    config::internal::{ConfigurableBase, SetOpt},
    config::*,
    default_headers::DefaultHeadersInterceptor,
    handler::{RequestHandler, ResponseBodyReader},
    headers,
    interceptor::{self, Interceptor, InterceptorObj},
    Body, Error,
};
use futures_lite::{future::block_on, io::AsyncRead, pin};
use http::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Request, Response,
};
use once_cell::sync::Lazy;
use std::{
    convert::TryFrom,
    fmt,
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tracing_futures::Instrument;

static USER_AGENT: Lazy<String> = Lazy::new(|| format!(
    "curl/{} isahc/{}",
    curl::Version::get().version(),
    env!("CARGO_PKG_VERSION")
));

/// An HTTP client builder, capable of creating custom [`HttpClient`] instances
/// with customized behavior.
///
/// Any option that can be configured per-request can also be configured on a
/// client builder as a default setting. Request configuration is provided by
/// the [`Configurable`] trait, which is also available in the
/// [`prelude`](crate::prelude) module.
///
/// # Examples
///
/// ```
/// use isahc::config::{RedirectPolicy, VersionNegotiation};
/// use isahc::prelude::*;
/// use std::time::Duration;
///
/// let client = HttpClient::builder()
///     .timeout(Duration::from_secs(60))
///     .redirect_policy(RedirectPolicy::Limit(10))
///     .version_negotiation(VersionNegotiation::http2())
///     .build()?;
/// # Ok::<(), isahc::Error>(())
/// ```
pub struct HttpClientBuilder {
    agent_builder: AgentBuilder,
    defaults: http::Extensions,
    interceptors: Vec<InterceptorObj>,
    default_headers: HeaderMap<HeaderValue>,
    error: Option<Error>,

    #[cfg(feature = "cookies")]
    cookie_jar: Option<crate::cookies::CookieJar>,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClientBuilder {
    /// Create a new builder for building a custom client. All configuration
    /// will start out with the default values.
    ///
    /// This is equivalent to the [`Default`] implementation.
    pub fn new() -> Self {
        let mut defaults = http::Extensions::new();

        // Always start out with latest compatible HTTP version.
        defaults.insert(VersionNegotiation::default());

        // Enable automatic decompression by default for convenience (and
        // maintain backwards compatibility).
        defaults.insert(AutomaticDecompression(true));

        // Erase curl's default auth method of Basic.
        defaults.insert(Authentication::default());

        Self {
            agent_builder: AgentBuilder::default(),
            defaults,
            interceptors: vec![
                // Add redirect support. Note that this is _always_ the first,
                // and thus the outermost, interceptor. Also note that this does
                // not enable redirect following, it just implements support for
                // it, if a request asks for it.
                InterceptorObj::new(crate::redirect::RedirectInterceptor),
            ],
            default_headers: HeaderMap::new(),
            error: None,

            #[cfg(feature = "cookies")]
            cookie_jar: None,
        }
    }

    /// Enable persistent cookie handling for all requests using this client
    /// using a shared cookie jar.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use isahc::prelude::*;
    /// #
    /// // Create a client with a cookie jar.
    /// let client = HttpClient::builder()
    ///     .cookies()
    ///     .build()?;
    ///
    /// // Make a request that sets a cookie.
    /// let uri = "http://httpbin.org/cookies/set?foo=bar".parse()?;
    /// client.get(&uri)?;
    ///
    /// // Get the cookie from the cookie jar.
    /// let cookie = client.cookie_jar()
    ///     .unwrap()
    ///     .get_by_name(&uri, "foo")
    ///     .unwrap();
    /// assert_eq!(cookie, "bar");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Availability
    ///
    /// This method is only available when the [`cookies`](index.html#cookies)
    /// feature is enabled.
    #[cfg(feature = "cookies")]
    pub fn cookies(self) -> Self {
        // Note: this method is now essentially the same as setting a default
        // cookie jar, but remains for backwards compatibility.
        self.cookie_jar(Default::default())
    }

    /// Add a request interceptor to the client.
    ///
    /// # Availability
    ///
    /// This method is only available when the
    /// [`unstable-interceptors`](index.html#unstable-interceptors) feature is
    /// enabled.
    #[cfg(feature = "unstable-interceptors")]
    #[inline]
    pub fn interceptor(self, interceptor: impl Interceptor + 'static) -> Self {
        self.interceptor_impl(interceptor)
    }

    #[allow(unused)]
    pub(crate) fn interceptor_impl(mut self, interceptor: impl Interceptor + 'static) -> Self {
        self.interceptors.push(InterceptorObj::new(interceptor));
        self
    }

    /// Set the maximum time-to-live (TTL) for connections to remain in the
    /// connection cache.
    ///
    /// After requests are completed, if the underlying connection is
    /// reusable, it is added to the connection cache to be reused to reduce
    /// latency for future requests. This option controls how long such
    /// connections should be still considered valid before being discarded.
    ///
    /// Old connections have a high risk of not working any more and thus
    /// attempting to use them wastes time if the server has disconnected.
    ///
    /// The default TTL is 118 seconds.
    pub fn connection_cache_ttl(mut self, ttl: Duration) -> Self {
        self.defaults.insert(MaxAgeConn(ttl));
        self
    }

    /// Set a maximum number of simultaneous connections that this client is
    /// allowed to keep open at one time.
    ///
    /// If set to a value greater than zero, no more than `max` connections will
    /// be opened at one time. If executing a new request would require opening
    /// a new connection, then the request will stay in a "pending" state until
    /// an existing connection can be used or an active request completes and
    /// can be closed, making room for a new connection.
    ///
    /// Setting this value to `0` disables the limit entirely.
    ///
    /// This is an effective way of limiting the number of sockets or file
    /// descriptors that this client will open, though note that the client may
    /// use file descriptors for purposes other than just HTTP connections.
    ///
    /// By default this value is `0` and no limit is enforced.
    ///
    /// To apply a limit per-host, see
    /// [`HttpClientBuilder::max_connections_per_host`].
    pub fn max_connections(mut self, max: usize) -> Self {
        self.agent_builder = self.agent_builder.max_connections(max);
        self
    }

    /// Set a maximum number of simultaneous connections that this client is
    /// allowed to keep open to individual hosts at one time.
    ///
    /// If set to a value greater than zero, no more than `max` connections will
    /// be opened to a single host at one time. If executing a new request would
    /// require opening a new connection, then the request will stay in a
    /// "pending" state until an existing connection can be used or an active
    /// request completes and can be closed, making room for a new connection.
    ///
    /// Setting this value to `0` disables the limit entirely. By default this
    /// value is `0` and no limit is enforced.
    ///
    /// To set a global limit across all hosts, see
    /// [`HttpClientBuilder::max_connections`].
    pub fn max_connections_per_host(mut self, max: usize) -> Self {
        self.agent_builder = self.agent_builder.max_connections_per_host(max);
        self
    }

    /// Set the size of the connection cache.
    ///
    /// After requests are completed, if the underlying connection is reusable,
    /// it is added to the connection cache to be reused to reduce latency for
    /// future requests.
    ///
    /// Setting the size to `0` disables connection caching for all requests
    /// using this client.
    ///
    /// By default this value is unspecified. A reasonable default size will be
    /// chosen.
    pub fn connection_cache_size(mut self, size: usize) -> Self {
        self.agent_builder = self.agent_builder.connection_cache_size(size);
        self.defaults.insert(CloseConnection(size == 0));
        self
    }

    /// Configure DNS caching.
    ///
    /// By default, DNS entries are cached by the client executing the request
    /// and are used until the entry expires. Calling this method allows you to
    /// change the entry timeout duration or disable caching completely.
    ///
    /// Note that DNS entry TTLs are not respected, regardless of this setting.
    ///
    /// By default caching is enabled with a 60 second timeout.
    ///
    /// # Examples
    ///
    /// ```
    /// # use isahc::config::*;
    /// # use isahc::prelude::*;
    /// # use std::time::Duration;
    /// #
    /// let client = HttpClient::builder()
    ///     // Cache entries for 10 seconds.
    ///     .dns_cache(Duration::from_secs(10))
    ///     // Cache entries forever.
    ///     .dns_cache(DnsCache::Forever)
    ///     // Don't cache anything.
    ///     .dns_cache(DnsCache::Disable)
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    pub fn dns_cache(self, cache: impl Into<DnsCache>) -> Self {
        // This option is per-request, but we only expose it on the client.
        // Since the DNS cache is shared between all requests, exposing this
        // option per-request would actually cause the timeout to alternate
        // values for every request with a different timeout, resulting in some
        // confusing (but valid) behavior.
        self.configure(cache.into())
    }

    /// Set a mapping of DNS resolve overrides.
    ///
    /// Entries in the given map will be used first before using the default DNS
    /// resolver for host+port pairs.
    ///
    /// Note that DNS resolving is only performed when establishing a new
    ///
    /// # Examples
    ///
    /// ```
    /// # use isahc::config::ResolveMap;
    /// # use isahc::prelude::*;
    /// # use std::net::IpAddr;
    /// #
    /// let client = HttpClient::builder()
    ///     .dns_resolve(ResolveMap::new()
    ///         // Send requests for example.org on port 80 to 127.0.0.1.
    ///         .add("example.org", 80, [127, 0, 0, 1]))
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn dns_resolve(self, map: ResolveMap) -> Self {
        // Similar to the dns_cache option, this operation actually affects all
        // requests in a multi handle so we do not expose it per-request to
        // avoid confusing behavior.
        self.configure(map)
    }

    /// Add a default header to be passed with every request.
    ///
    /// If a default header value is already defined for the given key, then a
    /// second header value will be appended to the list and multiple header
    /// values will be included in the request.
    ///
    /// If any values are defined for this header key on an outgoing request,
    /// they will override any default header values.
    ///
    /// If the header key or value are malformed, [`HttpClientBuilder::build`]
    /// will return an error.
    ///
    /// # Examples
    ///
    /// ```
    /// # use isahc::prelude::*;
    /// #
    /// let client = HttpClient::builder()
    ///     .default_header("some-header", "some-value")
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn default_header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        match HeaderName::try_from(key) {
            Ok(key) => match HeaderValue::try_from(value) {
                Ok(value) => {
                    self.default_headers.append(key, value);
                }
                Err(e) => {
                    self.error = Some(e.into().into());
                }
            },
            Err(e) => {
                self.error = Some(e.into().into());
            }
        }
        self
    }

    /// Set the default headers to include in every request, replacing any
    /// previously set default headers.
    ///
    /// Headers defined on an individual request always override headers in the
    /// default map.
    ///
    /// If any header keys or values are malformed, [`HttpClientBuilder::build`]
    /// will return an error.
    ///
    /// # Examples
    ///
    /// Set default headers from a slice:
    ///
    /// ```
    /// # use isahc::prelude::*;
    /// #
    /// let mut builder = HttpClient::builder()
    ///     .default_headers(&[
    ///         ("some-header", "value1"),
    ///         ("some-header", "value2"),
    ///         ("some-other-header", "some-other-value"),
    ///     ])
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Using an existing header map:
    ///
    /// ```
    /// # use isahc::prelude::*;
    /// #
    /// let mut headers = http::HeaderMap::new();
    /// headers.append("some-header".parse::<http::header::HeaderName>()?, "some-value".parse()?);
    ///
    /// let mut builder = HttpClient::builder()
    ///     .default_headers(&headers)
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Using a hashmap:
    ///
    /// ```
    /// # use isahc::prelude::*;
    /// # use std::collections::HashMap;
    /// #
    /// let mut headers = HashMap::new();
    /// headers.insert("some-header", "some-value");
    ///
    /// let mut builder = HttpClient::builder()
    ///     .default_headers(headers)
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn default_headers<K, V, I, P>(mut self, headers: I) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
        I: IntoIterator<Item = P>,
        P: HeaderPair<K, V>,
    {
        self.default_headers.clear();

        for (key, value) in headers.into_iter().map(HeaderPair::pair) {
            self = self.default_header(key, value);
        }

        self
    }

    /// Build an [`HttpClient`] using the configured options.
    ///
    /// If the client fails to initialize, an error will be returned.
    #[allow(unused_mut)]
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn build(mut self) -> Result<HttpClient, Error> {
        if let Some(err) = self.error {
            return Err(err);
        }

        // Add cookie interceptor if enabled.
        #[cfg(feature = "cookies")]
        {
            let jar = self.cookie_jar.clone();
            self = self.interceptor_impl(crate::cookies::interceptor::CookieInterceptor::new(jar));
        }

        // Add default header interceptor if any default headers were specified.
        if !self.default_headers.is_empty() {
            let default_headers = std::mem::take(&mut self.default_headers);
            self = self.interceptor_impl(DefaultHeadersInterceptor::from(default_headers));
        }

        let inner = InnerHttpClient {
            agent: self.agent_builder.spawn()?,
            defaults: self.defaults,
            interceptors: self.interceptors,

            #[cfg(feature = "cookies")]
            cookie_jar: self.cookie_jar,
        };

        Ok(HttpClient { inner: Arc::new(inner) })
    }
}

impl Configurable for HttpClientBuilder {
    #[cfg(feature = "cookies")]
    fn cookie_jar(mut self, cookie_jar: crate::cookies::CookieJar) -> Self {
        self.cookie_jar = Some(cookie_jar);
        self
    }
}

impl ConfigurableBase for HttpClientBuilder {
    fn configure(mut self, option: impl Send + Sync + 'static) -> Self {
        self.defaults.insert(option);
        self
    }
}

impl fmt::Debug for HttpClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClientBuilder").finish()
    }
}

/// Helper trait for defining key-value pair types that can be dereferenced into
/// a tuple from a reference.
///
/// This trait is sealed and cannot be implemented for types outside of Isahc.
pub trait HeaderPair<K, V> {
    fn pair(self) -> (K, V);
}

impl<K, V> HeaderPair<K, V> for (K, V) {
    fn pair(self) -> (K, V) {
        self
    }
}

impl<'a, K: Copy, V: Copy> HeaderPair<K, V> for &'a (K, V) {
    fn pair(self) -> (K, V) {
        (self.0, self.1)
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
/// let client = HttpClient::new()?;
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
/// use isahc::{
///     config::{RedirectPolicy, VersionNegotiation},
///     prelude::*,
/// };
/// use std::time::Duration;
///
/// let client = HttpClient::builder()
///     .version_negotiation(VersionNegotiation::http11())
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
#[derive(Clone)]
pub struct HttpClient {
    inner: Arc<InnerHttpClient>,
}

struct InnerHttpClient {
    /// This is how we talk to our background agent thread.
    agent: agent::Handle,

    /// Map of config values that should be used to configure execution if not
    /// specified in a request.
    defaults: http::Extensions,

    /// Registered interceptors that requests should pass through.
    interceptors: Vec<InterceptorObj>,

    /// Configured cookie jar, if any.
    #[cfg(feature = "cookies")]
    cookie_jar: Option<crate::cookies::CookieJar>,
}

impl HttpClient {
    /// Create a new HTTP client using the default configuration.
    ///
    /// If the client fails to initialize, an error will be returned.
    #[tracing::instrument(level = "debug")]
    pub fn new() -> Result<Self, Error> {
        HttpClientBuilder::default().build()
    }

    /// Get a reference to a global client instance.
    ///
    /// TODO: Stabilize.
    #[tracing::instrument(level = "debug")]
    pub(crate) fn shared() -> &'static Self {
        static SHARED: Lazy<HttpClient> = Lazy::new(|| HttpClient::new()
            .expect("shared client failed to initialize"));

        &SHARED
    }

    /// Create a new [`HttpClientBuilder`] for building a custom client.
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder::default()
    }

    /// Get the configured cookie jar for this HTTP client, if any.
    ///
    /// # Availability
    ///
    /// This method is only available when the [`cookies`](index.html#cookies)
    /// feature is enabled.
    #[cfg(feature = "cookies")]
    pub fn cookie_jar(&self) -> Option<&crate::cookies::CookieJar> {
        self.inner.cookie_jar.as_ref()
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
    /// # let client = HttpClient::new()?;
    /// let mut response = client.get("https://example.org")?;
    /// println!("{}", response.text()?);
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[inline]
    pub fn get<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        block_on(self.get_async(uri))
    }

    /// Send a GET request to the given URI asynchronously.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::get`].
    pub fn get_async<U>(&self, uri: U) -> ResponseFuture<'_>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
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
    /// # let client = HttpClient::new()?;
    /// let response = client.head("https://example.org")?;
    /// println!("Page size: {:?}", response.headers()["content-length"]);
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[inline]
    pub fn head<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        block_on(self.head_async(uri))
    }

    /// Send a HEAD request to the given URI asynchronously.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::head`].
    pub fn head_async<U>(&self, uri: U) -> ResponseFuture<'_>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
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
    /// let client = HttpClient::new()?;
    ///
    /// let response = client.post("https://httpbin.org/post", r#"{
    ///     "speed": "fast",
    ///     "cool_name": true
    /// }"#)?;
    /// # Ok::<(), isahc::Error>(())
    #[inline]
    pub fn post<U>(&self, uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        block_on(self.post_async(uri, body))
    }

    /// Send a POST request to the given URI asynchronously with a given request
    /// body.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::post`].
    pub fn post_async<U>(&self, uri: U, body: impl Into<Body>) -> ResponseFuture<'_>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.send_builder_async(http::Request::post(uri), body.into())
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
    /// let client = HttpClient::new()?;
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
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        block_on(self.put_async(uri, body))
    }

    /// Send a PUT request to the given URI asynchronously with a given request
    /// body.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::put`].
    pub fn put_async<U>(&self, uri: U, body: impl Into<Body>) -> ResponseFuture<'_>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.send_builder_async(http::Request::put(uri), body.into())
    }

    /// Send a DELETE request to the given URI.
    ///
    /// To customize the request further, see [`HttpClient::send`]. To execute
    /// the request asynchronously, see [`HttpClient::delete_async`].
    #[inline]
    pub fn delete<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        block_on(self.delete_async(uri))
    }

    /// Send a DELETE request to the given URI asynchronously.
    ///
    /// To customize the request further, see [`HttpClient::send_async`]. To
    /// execute the request synchronously, see [`HttpClient::delete`].
    pub fn delete_async<U>(&self, uri: U) -> ResponseFuture<'_>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.send_builder_async(http::Request::delete(uri), Body::empty())
    }

    /// Send an HTTP request and return the HTTP response.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    ///
    /// This client's configuration can be overridden for this request by
    /// configuring the request using methods provided by the [`Configurable`]
    /// trait.
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
    /// The response body is not a direct stream from the server, but uses its
    /// own buffering mechanisms internally for performance. It is therefore
    /// undesirable to wrap the body in additional buffering readers.
    ///
    /// To execute the request asynchronously, see [`HttpClient::send_async`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let client = HttpClient::new()?;
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
    #[tracing::instrument(level = "debug", skip(self, request), err)]
    pub fn send<B: Into<Body>>(&self, request: Request<B>) -> Result<Response<Body>, Error> {
        block_on(self.send_async(request))
    }

    /// Send an HTTP request and return the HTTP response asynchronously.
    ///
    /// See [`HttpClient::send`] for further details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), isahc::Error> {
    /// use isahc::prelude::*;
    ///
    /// let client = HttpClient::new()?;
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
    /// # Ok(()) }
    /// ```
    #[inline]
    pub fn send_async<B: Into<Body>>(&self, request: Request<B>) -> ResponseFuture<'_> {
        ResponseFuture::new(self.send_async_inner(request.map(Into::into)))
    }

    #[inline]
    fn send_builder_async(
        &self,
        builder: http::request::Builder,
        body: Body,
    ) -> ResponseFuture<'_> {
        ResponseFuture::new(async move { self.send_async_inner(builder.body(body)?).await })
    }

    /// Actually send the request. All the public methods go through here.
    async fn send_async_inner(&self, mut request: Request<Body>) -> Result<Response<Body>, Error> {
        let span = tracing::debug_span!(
            "send_async",
            method = ?request.method(),
            uri = ?request.uri(),
        );

        // Set redirect policy if not specified.
        if request.extensions().get::<RedirectPolicy>().is_none() {
            if let Some(policy) = self.inner.defaults.get::<RedirectPolicy>().cloned() {
                request.extensions_mut().insert(policy);
            }
        }

        let ctx = interceptor::Context {
            invoker: Arc::new(self),
            interceptors: &self.inner.interceptors,
        };

        ctx.send(request)
            .instrument(span)
            .await
    }

    fn create_easy_handle(
        &self,
        mut request: Request<Body>,
    ) -> Result<
        (
            curl::easy::Easy2<RequestHandler>,
            impl Future<Output = Result<Response<ResponseBodyReader>, Error>>,
        ),
        Error,
    > {
        // Prepare the request plumbing.
        // let (mut parts, body) = request.into_parts();
        let body = std::mem::take(request.body_mut());
        let has_body = !body.is_empty();
        let body_length = body.len();
        let (handler, future) = RequestHandler::new(body);

        let mut easy = curl::easy::Easy2::new(handler);

        // Set whether curl should generate verbose debug data for us to log.
        easy.verbose(easy.get_ref().is_debug_enabled())?;

        easy.signal(false)?;

        // Macro to apply all config values given in the request or in defaults.
        macro_rules! set_opts {
            ($easy:expr, $extensions:expr, $defaults:expr, [$($option:ty,)*]) => {{
                $(
                    if let Some(extension) = $extensions.get::<$option>().or_else(|| $defaults.get()) {
                        extension.set_opt($easy)?;
                    }
                )*
            }};
        }

        set_opts!(
            &mut easy,
            request.extensions(),
            self.inner.defaults,
            [
                Timeout,
                ConnectTimeout,
                TcpKeepAlive,
                TcpNoDelay,
                NetworkInterface,
                Dialer,
                AutomaticDecompression,
                Authentication,
                Credentials,
                MaxAgeConn,
                MaxUploadSpeed,
                MaxDownloadSpeed,
                VersionNegotiation,
                proxy::Proxy<Option<http::Uri>>,
                proxy::Blacklist,
                proxy::Proxy<Authentication>,
                proxy::Proxy<Credentials>,
                DnsCache,
                dns::ResolveMap,
                dns::Servers,
                ssl::Ciphers,
                ClientCertificate,
                CaCertificate,
                SslOption,
                CloseConnection,
                EnableMetrics,
                IpVersion,
            ]
        );

        // Set the HTTP method to use. Curl ties in behavior with the request
        // method, so we need to configure this carefully.
        #[allow(indirect_structural_match)]
        match (request.method(), has_body) {
            // Normal GET request.
            (&http::Method::GET, false) => {
                easy.get(true)?;
            }
            // Normal HEAD request.
            (&http::Method::HEAD, false) => {
                easy.nobody(true)?;
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

        easy.url(&uri_to_string(request.uri()))?;

        // If the request has a body, then we either need to tell curl how large
        // the body is if we know it, or tell curl to use chunked encoding. If
        // we do neither, curl will simply not send the body without warning.
        if has_body {
            // Use length given in Content-Length header, or the size defined by
            // the body itself.
            let body_length = request
                .headers()
                .get("Content-Length")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok())
                .or(body_length);

            if let Some(len) = body_length {
                if request.method() == http::Method::POST {
                    easy.post_field_size(len)?;
                } else {
                    easy.in_filesize(len)?;
                }
            } else {
                // Set the Transfer-Encoding header to instruct curl to use
                // chunked encoding. Replaces any existing values that may be
                // incorrect.
                request.headers_mut().insert(
                    "Transfer-Encoding",
                    http::header::HeaderValue::from_static("chunked"),
                );
            }
        }

        // Generate a header list for curl.
        let mut headers = curl::easy::List::new();

        let title_case = request
            .extensions()
            .get::<TitleCaseHeaders>()
            .or_else(|| self.inner.defaults.get())
            .map(|v| v.0)
            .unwrap_or(false);

        for (name, value) in request.headers().iter() {
            headers.append(&headers::to_curl_string(name, value, title_case))?;
        }

        easy.http_headers(headers)?;

        Ok((easy, future))
    }
}

impl crate::interceptor::Invoke for &HttpClient {
    fn invoke<'a>(&'a self, mut request: Request<Body>) -> crate::interceptor::InterceptorFuture<'a, Error> {
        Box::pin(
            async move {
                // Set default user agent if not specified.
                request
                    .headers_mut()
                    .entry(http::header::USER_AGENT)
                    .or_insert(USER_AGENT.parse().unwrap());

                // Check if automatic decompression is enabled; we'll need to
                // know this later after the response is sent.
                let is_automatic_decompression = request.extensions().get()
                    .or_else(|| self.inner.defaults.get())
                    .map(|AutomaticDecompression(enabled)| *enabled)
                    .unwrap_or(false);

                // Create and configure a curl easy handle to fulfil the request.
                let (easy, future) = self.create_easy_handle(request)?;

                // Send the request to the agent to be executed.
                self.inner.agent.submit_request(easy)?;

                // Await for the response headers.
                let response = future.await?;

                // If a Content-Length header is present, include that information in
                // the body as well.
                let content_length = response
                    .headers()
                    .get(http::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse().ok())
                    .filter(|_| {
                        // If automatic decompression is enabled, and will
                        // likely be selected, then the value of Content-Length
                        // does not indicate the uncompressed body length and
                        // merely the compressed data length. If it looks like
                        // we are in this scenario then we ignore the
                        // Content-Length, since it can only cause confusion
                        // when included with the body.
                        if is_automatic_decompression {
                            if let Some(value) = response.headers().get(http::header::CONTENT_ENCODING) {
                                if value != "identity" {
                                    return false;
                                }
                            }
                        }

                        true
                    });

                // Convert the reader into an opaque Body.
                Ok(response.map(|reader| {
                    let body = ResponseBody {
                        inner: reader,
                        // Extend the lifetime of the agent by including a reference
                        // to its handle in the response body.
                        _client: (*self).clone(),
                    };

                    if let Some(len) = content_length {
                        Body::from_reader_sized(body, len)
                    } else {
                        Body::from_reader(body)
                    }
                }))
            }
        )
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient").finish()
    }
}

/// A future for a request being executed.
pub struct ResponseFuture<'c>(Pin<Box<dyn Future<Output = Result<Response<Body>, Error>> + 'c + Send>>);

impl<'c> ResponseFuture<'c> {
    fn new(future: impl Future<Output = Result<Response<Body>, Error>> + Send + 'c) -> Self {
        ResponseFuture(Box::pin(future))
    }
}

impl Future for ResponseFuture<'_> {
    type Output = Result<Response<Body>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}

impl<'c> fmt::Debug for ResponseFuture<'c> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}

/// Response body stream. Holds a reference to the agent to ensure it is kept
/// alive until at least this transfer is complete.
struct ResponseBody {
    inner: ResponseBodyReader,
    _client: HttpClient,
}

impl AsyncRead for ResponseBody {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let inner = &mut self.inner;
        pin!(inner);
        inner.poll_read(cx, buf)
    }
}

/// Convert a URI to a string. This implementation is a bit faster than the
/// `Display` implementation that avoids the `std::fmt` machinery.
fn uri_to_string(uri: &http::Uri) -> String {
    let mut s = String::new();

    if let Some(scheme) = uri.scheme() {
        s.push_str(scheme.as_str());
        s.push_str("://");
    }

    if let Some(authority) = uri.authority() {
        s.push_str(authority.as_str());
    }

    s.push_str(uri.path());

    if let Some(query) = uri.query() {
        s.push('?');
        s.push_str(query);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(HttpClient: Send, Sync);
    static_assertions::assert_impl_all!(HttpClientBuilder: Send);

    #[test]
    fn test_default_header() {
        let client = HttpClientBuilder::new()
            .default_header("some-key", "some-value")
            .build();
        match client {
            Ok(_) => assert!(true),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn test_default_headers_mut() {
        let mut builder = HttpClientBuilder::new().default_header("some-key", "some-value");
        let headers_map = &mut builder.default_headers;
        assert!(headers_map.len() == 1);

        let mut builder = HttpClientBuilder::new()
            .default_header("some-key", "some-value1")
            .default_header("some-key", "some-value2");
        let headers_map = &mut builder.default_headers;

        assert!(headers_map.len() == 2);

        let mut builder = HttpClientBuilder::new();
        let header_map = &mut builder.default_headers;
        assert!(header_map.is_empty())
    }
}
