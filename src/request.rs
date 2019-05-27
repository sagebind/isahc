use crate::{Body, Error};
use crate::options::*;
use http::{Request, Response};
use std::time::Duration;

/// Extension methods on an HTTP request builder.
pub trait RequestBuilderExt {
    fn timeout(&mut self, timeout: impl Into<Timeout>) -> &mut Self;
}

/// Extension methods on an HTTP request.
pub trait RequestExt {
    /// Send the HTTP request and return the response synchronously.
    ///
    /// The response body is provided as a stream that may only be consumed once.
    fn send(self) -> Result<Response<Body>, Error>;
}

impl RequestBuilderExt for http::request::Builder {
    fn timeout(&mut self, timeout: impl Into<Timeout>) -> &mut Self {
        self.extension(timeout.into())
    }
}

impl<T> RequestExt for http::Request<T> where T: Into<Body> {
    fn send(self) -> Result<Response<Body>, Error> {
        crate::send(self)
    }
}


pub trait Configurable {
    fn set_option<T: Send + Sync + 'static>(&mut self, option: T) -> &mut Self;

    fn timeout(&mut self, timeout: impl Into<Timeout>) -> &mut Self {
        self.set_option(timeout.into())
    }
}
