//! The HTTP client implementation.

use body::Body;
use error::Error;
use futures::prelude::*;
use http::{self, Request, Response};
use internal::agent::CurlAgent;
use internal::request;
use options::*;
use std::sync::Mutex;

/// An HTTP client builder.
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
        let agent = CurlAgent::new()?;

        Ok(Client {
            agent: Mutex::new(agent),
            options: self.options,
        })
    }
}

/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to create, so we recommend re-using your clients
/// instead of discarding and recreating them.
pub struct Client {
    agent: Mutex<CurlAgent>,
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
        let request = http::Request::get(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a HEAD request.
    pub fn head(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::head(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a POST request.
    pub fn post<B: Into<Body>>(&self, uri: &str, body: B) -> Result<Response<Body>, Error> {
        let request = http::Request::post(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a PUT request.
    pub fn put<B: Into<Body>>(&self, uri: &str, body: B) -> Result<Response<Body>, Error> {
        let request = http::Request::put(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a DELETE request.
    pub fn delete(&self, uri: &str) -> Result<Response<Body>, Error> {
        let request = http::Request::delete(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a request and returns the response.
    pub fn send(&self, request: Request<Body>) -> Result<Response<Body>, Error> {
        self.send_async(request).wait()
    }

    /// Sends a request and returns the response.
    pub fn send_async(&self, request: Request<Body>) -> impl Future<Item=Response<Body>, Error=Error> {
        let (request, future) = request::create(request, &self.options).unwrap();

        {
            let mut agent = self.agent.lock().unwrap();
            agent.begin_execute(request).unwrap();
        }

        future
    }
}
