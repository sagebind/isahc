//! The HTTP client implementation.

use body::Body;
use error::Error;
use futures::executor;
use futures::prelude::*;
use http::{self, Request, Response};
use internal::agent;
use internal::request;
use options::*;

/// An HTTP client builder.
///
/// This builder can be used to create an HTTP client with customized behavior.
#[derive(Clone)]
pub struct ClientBuilder {
    options: Options,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            options: Default::default(),
        }
    }
}

impl ClientBuilder {
    /// Set the connection options to use.
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    /// Build an HTTP client using the configured options.
    pub fn build(self) -> Result<Client, Error> {
        let agent = agent::create()?;

        Ok(Client {
            agent: agent,
            options: self.options,
        })
    }
}

/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to create, so we recommend re-using your clients
/// instead of discarding and recreating them.
pub struct Client {
    agent: agent::Handle,
    options: Options,
}

impl Client {
    /// Create a new HTTP client using the default configuration.
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Create a new HTTP client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Sends a GET request.
    pub fn get(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::get(uri).body(Body::default())?;
        self.send(request)
    }

    /// Sends a HEAD request.
    pub fn head(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::head(uri).body(Body::default())?;
        self.send(request)
    }

    /// Sends a POST request.
    pub fn post(&self, uri: &str, body: impl Into<Body>) -> Result<Response<Body>, Error> {
        let request = http::Request::post(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a PUT request.
    pub fn put(&self, uri: &str, body: impl Into<Body>) -> Result<Response<Body>, Error> {
        let request = http::Request::put(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a DELETE request.
    pub fn delete(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::delete(uri).body(Body::default())?;
        self.send(request)
    }

    /// Sends a request and returns the response.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    pub fn send(&self, request: Request<Body>) -> Result<Response<Body>, Error> {
        executor::block_on(self.send_async(request))
    }

    /// Begin sending a request and return a future of the response.
    fn send_async(&self, request: Request<Body>) -> impl Future<Item=Response<Body>, Error=Error> {
        return request::create(request, &self.options)
            .and_then(|(request, future)| {
                self.agent.begin_execute(request).map(|_| future)
            })
            .into_future()
            .flatten();
    }
}
