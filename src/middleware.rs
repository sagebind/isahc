//! HTTP client middleware API.
//!
//! This module provides the core types and functions for defining and working with middleware. Middleware are handlers
//! that augment HTTP client functionality by applying transformations to HTTP requests before they are sent and/or HTTP
//! responses after they are received.

use super::Request;
use super::Response;

/// Create a new _before_ middleware from a function.
pub fn before(f: impl Fn(Request) -> Request + Send + Sync + 'static) -> impl Middleware {
    create(f, |res| res)
}

/// Create a new _after_ middleware from a function.
pub fn after(f: impl Fn(Response) -> Response + Send + Sync + 'static) -> impl Middleware {
    create(|req| req, f)
}

/// Create a new middleware from a pair of functions.
pub fn create(
    before: impl Fn(Request) -> Request + Send + Sync + 'static,
    after: impl Fn(Response) -> Response + Send + Sync + 'static,
) -> impl Middleware {
    struct Impl<F, G>(F, G);

    impl<F, G> Middleware for Impl<F, G>
    where
        F: Fn(Request) -> Request + Send + Sync + 'static,
        G: Fn(Response) -> Response + Send + Sync + 'static,
    {
        fn before(&self, request: Request) -> Request {
            (self.0)(request)
        }

        fn after(&self, response: Response) -> Response {
            (self.1)(response)
        }
    }

    Impl(before, after)
}

/// Base trait for middleware.
///
/// Since clients may be used to send requests concurrently, all middleware must be synchronized and must be able to
/// account for multiple requests being made in parallel.
pub trait Middleware: Send + Sync + 'static {
    /// Transform a request before it is sent.
    fn before(&self, request: Request) -> Request {
        request
    }

    /// Transform a response after it is received.
    fn after(&self, response: Response) -> Response {
        response
    }
}
