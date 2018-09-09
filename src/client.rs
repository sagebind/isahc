//! The HTTP client implementation.

use body::Body;
use error::Error;
use futures::executor;
use futures::prelude::*;
use http::{self, Request, Response};
use internal::agent;
use internal::request;
use options::*;

/// An HTTP client builder, capable of creating custom [`Client`](struct.Client.html) instances with customized
/// behavior.
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
#[derive(Clone, Default)]
pub struct ClientBuilder {
    default_options: Options,
}

impl ClientBuilder {
    /// Set the default connection options to use for each request.
    ///
    /// If a request has custom options, then they will override any options specified here.
    pub fn options(&mut self, options: Options) -> &mut Self {
        self.default_options = options;
        self
    }

    /// Build an HTTP client using the configured options.
    ///
    /// If the client fails to initialize, an error will be returned.
    pub fn build(&self) -> Result<Client, Error> {
        let agent = agent::create()?;

        Ok(Client {
            agent: agent,
            default_options: self.default_options.clone(),
        })
    }
}

/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to create, so we recommend re-using your clients
/// instead of discarding and recreating them.
pub struct Client {
    agent: agent::Handle,
    default_options: Options,
}

impl Client {
    /// Create a new HTTP client using the default configuration.
    ///
    /// If the client fails to initialize, an error will be returned.
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Create a new builder for building a custom client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Sends an HTTP GET request.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    pub fn get(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::get(uri).body(Body::default())?;
        self.send(request)
    }

    /// Sends an HTTP HEAD request.
    pub fn head(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::head(uri).body(Body::default())?;
        self.send(request)
    }

    /// Sends an HTTP POST request.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    pub fn post(&self, uri: &str, body: impl Into<Body>) -> Result<Response<Body>, Error> {
        let request = http::Request::post(uri).body(body)?;
        self.send(request)
    }

    /// Sends an HTTP PUT request.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    pub fn put(&self, uri: &str, body: impl Into<Body>) -> Result<Response<Body>, Error> {
        let request = http::Request::put(uri).body(body)?;
        self.send(request)
    }

    /// Sends an HTTP DELETE request.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    pub fn delete(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::delete(uri).body(Body::default())?;
        self.send(request)
    }

    /// Sends a request and returns the response.
    ///
    /// The request may include [extensions](../../http/struct.Extensions.html) to customize how it is sent. If the
    /// request contains an [`Options`](chttp::options::Options) struct as an extension, then those options will be used
    /// instead of the default options this client is configured with.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    pub fn send<B: Into<Body>>(&self, request: Request<B>) -> Result<Response<Body>, Error> {
        executor::block_on(self.send_async(request))
    }

    /// Begin sending a request and return a future of the response.
    fn send_async<B: Into<Body>>(&self, request: Request<B>) -> impl Future<Item=Response<Body>, Error=Error> {
        return request::create(request, &self.default_options)
            .and_then(|(request, future)| {
                self.agent.begin_execute(request).map(|_| future)
            })
            .into_future()
            .flatten();
    }
}
