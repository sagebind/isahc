//! HTTP client middleware API.
//!
//! This module provides the core types and functions for defining and working
//! with middleware. Middleware are handlers that augment HTTP client
//! functionality by applying transformations to HTTP requests before they are
//! sent and/or HTTP responses after they are received.

use crate::Body;
use http::{Request, Response};
use std::convert::identity;

/// Create a new _request_ middleware from a function.
pub fn before(
    f: impl Fn(Request<Body>) -> Request<Body> + Send + Sync + 'static,
) -> impl Middleware {
    create(f, identity)
}

/// Create a new _response_ middleware from a function.
pub fn after(
    f: impl Fn(Response<Body>) -> Response<Body> + Send + Sync + 'static,
) -> impl Middleware {
    create(identity, f)
}

/// Create a new middleware from a pair of functions.
pub fn create(
    request: impl Fn(Request<Body>) -> Request<Body> + Send + Sync + 'static,
    response: impl Fn(Response<Body>) -> Response<Body> + Send + Sync + 'static,
) -> impl Middleware {
    struct Impl<F, G>(F, G);

    impl<F, G> Middleware for Impl<F, G>
    where
        F: Fn(Request<Body>) -> Request<Body> + Send + Sync + 'static,
        G: Fn(Response<Body>) -> Response<Body> + Send + Sync + 'static,
    {
        fn filter_request(&self, request: Request<Body>) -> Request<Body> {
            (self.0)(request)
        }

        fn filter_response(&self, response: Response<Body>) -> Response<Body> {
            (self.1)(response)
        }
    }

    Impl(request, response)
}

/// Base trait for middleware.
///
/// Since clients may be used to send requests concurrently, all middleware must
/// be synchronized and must be able to account for multiple requests being made
/// in parallel.
pub trait Middleware: Send + Sync + 'static {
    /// Transform a request before it is sent.
    fn filter_request(&self, request: Request<Body>) -> Request<Body> {
        request
    }

    /// Transform a response after it is received.
    fn filter_response(&self, response: Response<Body>) -> Response<Body> {
        response
    }
}
