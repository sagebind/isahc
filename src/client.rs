//! The HTTP client implementation.

use crate::handler::*;
use crate::middleware::Middleware;
use crate::options::*;
use crate::{agent, Body, Error};
use futures::executor::block_on;
use futures::prelude::*;
use http::{Request, Response};
use lazy_static::lazy_static;
use std::fmt;
use std::sync::Arc;

lazy_static! {
    static ref USER_AGENT: String = format!(
        "curl/{} chttp/{}",
        curl::Version::get().version(),
        env!("CARGO_PKG_VERSION")
    );
}

/// An HTTP client builder, capable of creating custom [`Client`] instances with
/// customized behavior.
///
/// Example:
///
/// ```rust
/// use chttp::{http, Client, Options, RedirectPolicy};
/// use std::time::Duration;
///
/// # fn run() -> Result<(), chttp::Error> {
/// let client = Client::builder()
///     .options(Options::default()
///         .with_timeout(Some(Duration::from_secs(60)))
///         .with_redirect_policy(RedirectPolicy::Limit(10))
///         .with_preferred_http_version(Some(http::Version::HTTP_2)))
///     .build()?;
///
/// let mut response = client.get("https://example.org")?;
/// let body = response.body_mut().text()?;
/// println!("{}", body);
/// # Ok(())
/// # }
/// ```
pub struct ClientBuilder {
    default_options: Options,
    middleware: Vec<Box<dyn Middleware>>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    /// Create a new builder for building a custom client.
    pub fn new() -> Self {
        Self {
            default_options: Options::default(),
            middleware: Vec::new(),
        }
    }

    /// Set the default connection options to use for each request.
    ///
    /// If a request has custom options, then they will override any options
    /// specified here.
    pub fn options(mut self, options: Options) -> Self {
        self.default_options = options;
        self
    }

    /// Enable persistent cookie handling using a cookie jar.
    #[cfg(feature = "cookies")]
    pub fn with_cookies(self) -> Self {
        self.with_middleware_impl(crate::cookies::CookieJar::default())
    }

    /// Add a middleware layer to the client.
    #[cfg(feature = "middleware-api")]
    pub fn with_middleware(self, middleware: impl Middleware) -> Self {
        self.with_middleware_impl(middleware)
    }

    #[allow(unused)]
    fn with_middleware_impl(mut self, middleware: impl Middleware) -> Self {
        self.middleware.push(Box::new(middleware));
        self
    }

    /// Build an HTTP client using the configured options.
    ///
    /// If the client fails to initialize, an error will be returned.
    pub fn build(&mut self) -> Result<Client, Error> {
        let agent = agent::new()?;

        Ok(Client {
            agent: agent,
            default_options: self.default_options.clone(),
            middleware: Arc::new(self.middleware.drain(..).collect()),
        })
    }
}

/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to
/// create, so we recommend re-using your clients instead of discarding and
/// recreating them.
pub struct Client {
    agent: agent::Handle,
    default_options: Options,
    middleware: Arc<Vec<Box<dyn Middleware>>>,
}

impl Client {
    /// Create a new HTTP client using the default configuration.
    ///
    /// Panics if any internal systems failed to initialize during creation.
    /// This might occur if creating a socket fails, spawning a thread fails, or
    /// if something else goes wrong.
    pub fn new() -> Self {
        ClientBuilder::default()
            .build()
            .expect("client failed to initialize")
    }

    /// Get a reference to a global client instance.
    pub(crate) fn shared() -> &'static Self {
        lazy_static! {
            static ref CLIENT: Client = Client::new();
        }
        &CLIENT
    }

    /// Create a new builder for building a custom client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Sends an HTTP GET request.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    pub fn get<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        block_on(self.get_async(uri))
    }

    /// Sends an HTTP GET request asynchronously.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    #[auto_enums::auto_enum(Future)]
    pub fn get_async<U>(&self, uri: U) -> impl Future<Output = Result<Response<Body>, Error>>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        match http::Request::get(uri).body(Body::empty()) {
            Ok(request) => self.send_async(request),
            Err(e) => future::err(e.into()),
        }
    }

    /// Sends an HTTP HEAD request.
    pub fn head<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        block_on(self.head_async(uri))
    }

    /// Sends an HTTP HEAD request asynchronously.
    #[auto_enums::auto_enum(Future)]
    pub fn head_async<U>(&self, uri: U) -> impl Future<Output = Result<Response<Body>, Error>>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        match http::Request::head(uri).body(Body::empty()) {
            Ok(request) => self.send_async(request),
            Err(e) => future::err(e.into()),
        }
    }

    /// Sends an HTTP POST request.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    pub fn post<U>(&self, uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        block_on(self.post_async(uri, body))
    }

    /// Sends an HTTP POST request asynchronously.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    #[auto_enums::auto_enum(Future)]
    pub fn post_async<U>(
        &self,
        uri: U,
        body: impl Into<Body>,
    ) -> impl Future<Output = Result<Response<Body>, Error>>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        match http::Request::post(uri).body(body) {
            Ok(request) => self.send_async(request),
            Err(e) => future::err(e.into()),
        }
    }

    /// Sends an HTTP PUT request.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    pub fn put<U>(&self, uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        block_on(self.put_async(uri, body))
    }

    /// Sends an HTTP PUT request asynchronously.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    #[auto_enums::auto_enum(Future)]
    pub fn put_async<U>(
        &self,
        uri: U,
        body: impl Into<Body>,
    ) -> impl Future<Output = Result<Response<Body>, Error>>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        match http::Request::put(uri).body(body) {
            Ok(request) => self.send_async(request),
            Err(e) => future::err(e.into()),
        }
    }

    /// Sends an HTTP DELETE request.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    pub fn delete<U>(&self, uri: U) -> Result<Response<Body>, Error>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        block_on(self.delete_async(uri))
    }

    /// Sends an HTTP DELETE request asynchronously.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    #[auto_enums::auto_enum(Future)]
    pub fn delete_async<U>(&self, uri: U) -> impl Future<Output = Result<Response<Body>, Error>>
    where
        http::Uri: http::HttpTryFrom<U>,
    {
        match http::Request::delete(uri).body(Body::empty()) {
            Ok(request) => self.send_async(request),
            Err(e) => future::err(e.into()),
        }
    }

    /// Sends a request and returns the response.
    ///
    /// The request may include [extensions](http::Extensions) to customize how
    /// it is sent. If the request contains an [`Options`] struct as an
    /// extension, then those options will be used instead of the default
    /// options this client is configured with.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    pub fn send<B: Into<Body>>(&self, request: Request<B>) -> Result<Response<Body>, Error> {
        block_on(self.send_async(request))
    }

    /// Begin sending a request and return a future of the response.
    ///
    /// The request may include [extensions](http::Extensions) to customize how
    /// it is sent. If the request contains an [`Options`] struct as an
    /// extension, then those options will be used instead of the default
    /// options this client is configured with.
    ///
    /// The response body is provided as a stream that may only be consumed
    /// once.
    pub fn send_async<B: Into<Body>>(
        &self,
        request: Request<B>,
    ) -> impl Future<Output = Result<Response<Body>, Error>> {
        let mut request = request.map(Into::into);
        let uri = request.uri().clone();

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

        let middleware = self.middleware.clone();

        // Create and configure a curl easy handle to fulfil the request.
        let result = self
            .create_easy_handle(request)
            // Send the request to the agent to be executed.
            .and_then(|(easy, future)| self.agent.submit_request(easy).map(move |_| future));

        future::ready(result)
            .and_then(|future| future)
            .map(move |result| {
                result.map(move |mut response| {
                    response.extensions_mut().insert(uri);

                    // Apply response middleware, starting with the innermost
                    // one.
                    for middleware in middleware.iter() {
                        response = middleware.filter_response(response);
                    }

                    response
                })
            })
    }

    fn create_easy_handle(
        &self,
        mut request: Request<Body>,
    ) -> Result<(curl::easy::Easy2<RequestHandler>, ResponseFuture), Error> {
        // Extract the request options, or use the default options.
        let options = request.extensions_mut().remove::<Options>();
        let options = options.as_ref().unwrap_or(&self.default_options);

        // Prepare the request plumbing.
        let (request_parts, request_body) = request.into_parts();
        let body_is_empty = request_body.is_empty();
        let body_size = request_body.len();
        let (handler, future) = RequestHandler::new(request_body);

        let mut easy = curl::easy::Easy2::new(handler);

        easy.verbose(log::log_enabled!(log::Level::Trace))?;
        easy.signal(false)?;
        easy.buffer_size(options.buffer_size)?;

        if let Some(timeout) = options.timeout {
            easy.timeout(timeout)?;
        }

        easy.connect_timeout(options.connect_timeout)?;

        easy.tcp_nodelay(options.tcp_nodelay)?;
        if let Some(interval) = options.tcp_keepalive {
            easy.tcp_keepalive(true)?;
            easy.tcp_keepintvl(interval)?;
        } else {
            easy.tcp_keepalive(false)?;
        }

        match options.redirect_policy {
            RedirectPolicy::None => {
                easy.follow_location(false)?;
            }
            RedirectPolicy::Follow => {
                easy.follow_location(true)?;
            }
            RedirectPolicy::Limit(max) => {
                easy.follow_location(true)?;
                easy.max_redirections(max)?;
            }
        }

        if let Some(limit) = options.max_upload_speed {
            easy.max_send_speed(limit)?;
        }

        if let Some(limit) = options.max_download_speed {
            easy.max_recv_speed(limit)?;
        }

        // Set a preferred HTTP version to negotiate.
        easy.http_version(match options.preferred_http_version {
            Some(http::Version::HTTP_10) => curl::easy::HttpVersion::V10,
            Some(http::Version::HTTP_11) => curl::easy::HttpVersion::V11,
            Some(http::Version::HTTP_2) => curl::easy::HttpVersion::V2,
            _ => curl::easy::HttpVersion::Any,
        })?;

        if let Some(ref proxy) = options.proxy {
            easy.proxy(&format!("{}", proxy))?;
        }

        if let Some(addrs) = &options.dns_servers {
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
        if let Some(ciphers) = &options.ssl_ciphers {
            easy.ssl_cipher_list(&ciphers.join(":"))?;
        }
        if let Some(cert) = &options.ssl_client_certificate {
            easy.ssl_client_certificate(cert)?;
        }

        // Enable automatic response decompression.
        easy.accept_encoding("")?;

        // Set the request data according to the request given.
        easy.custom_request(request_parts.method.as_str())?;
        easy.url(&request_parts.uri.to_string())?;

        let mut headers = curl::easy::List::new();
        for (name, value) in request_parts.headers.iter() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header)?;
        }
        easy.http_headers(headers)?;

        // If the request body is non-empty, tell curl that we are going to
        // upload something.
        if !body_is_empty {
            easy.upload(true)?;

            if let Some(len) = body_size {
                // If we know the size of the request body up front, tell curl
                // about it.
                easy.in_filesize(len as u64)?;
            }
        }

        Ok((easy, future))
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Client")
            .field("default_options", &self.default_options)
            .field("middleware", &self.middleware.len())
            .finish()
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

static_assertions::assert_impl!(client; Client, Send, Sync);
static_assertions::assert_impl!(builder; ClientBuilder, Send);
