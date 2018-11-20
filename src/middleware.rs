//! middleware

use super::Request;
use super::Response;

/// fo
pub trait Middleware: 'static {
    /// Transform a request before it is sent.
    fn before(&self, request: Request) -> Request {
        request
    }

    /// Transform a response after it is received.
    fn after(&self, response: Response) -> Response {
        response
    }
}
