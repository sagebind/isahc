//! The HTTP client implementation.

use agent::CurlAgent;
use body::Body;
use curl;
use error::Error;
use futures;
use futures::future;
use futures::prelude::*;
use http::{self, Request, Response};
use options::*;
use std::io;
use std::io::Read;
use std::time::Duration;
use transfer::CurlTransfer;

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
    agent: CurlAgent,
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
        let mut transfer = self.create_transfer(request).unwrap();

        let future = transfer.future.take().unwrap().then(|result| match result {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => Err(e),
            Err(futures::sync::oneshot::Canceled) => Err(Error::Canceled),
        }).map(|response| {
            response.map(|body| {
                Body::from_reader(body)
            })
        });

        self.agent.add(transfer);

        future
    }

    fn create_transfer(&self, request: Request<Body>) -> Result<CurlTransfer, Error> {
        let mut transfer = CurlTransfer::new()?;

        if let Some(timeout) = self.options.timeout {
            transfer.easy.timeout(timeout)?;
        }

        transfer.easy.connect_timeout(self.options.connect_timeout)?;

        transfer.easy.tcp_nodelay(self.options.tcp_nodelay)?;
        if let Some(interval) = self.options.tcp_keepalive {
            transfer.easy.tcp_keepalive(true)?;
            transfer.easy.tcp_keepintvl(interval)?;
        }

        // Configure redirects.
        match self.options.redirect_policy {
            RedirectPolicy::None => {
                transfer.easy.follow_location(false)?;
            }
            RedirectPolicy::Follow => {
                transfer.easy.follow_location(true)?;
            }
            RedirectPolicy::Limit(max) => {
                transfer.easy.follow_location(true)?;
                transfer.easy.max_redirections(max)?;
            }
        }

        // Set a preferred HTTP version to negotiate.
        if let Some(version) = self.options.preferred_http_version {
            transfer.easy.http_version(match version {
                http::Version::HTTP_10 => curl::easy::HttpVersion::V10,
                http::Version::HTTP_11 => curl::easy::HttpVersion::V11,
                http::Version::HTTP_2 => curl::easy::HttpVersion::V2,
                _ => curl::easy::HttpVersion::Any,
            })?;
        }

        // Set a proxy to use.
        if let Some(ref proxy) = self.options.proxy {
            transfer.easy.proxy(&format!("{}", proxy))?;
        }

        // Set the request data according to the request given.
        transfer.easy.custom_request(request.method().as_str())?;
        transfer.easy.url(&format!("{}", request.uri()))?;

        let mut headers = curl::easy::List::new();
        for (name, value) in request.headers() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header)?;
        }
        transfer.easy.http_headers(headers)?;

        // Set the request body.
        let body = request.into_parts().1;
        if !body.is_empty() {
            transfer.easy.upload(true)?;
        }
        *transfer.request_body_mut() = body;

        Ok(transfer)
    }
}
