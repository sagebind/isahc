use crate::{
    request::Request,
    response::Response,
};

/// A responder is a request-response handler responsible for producing the
/// responses returned by a mock endpoint.
///
/// Responders are not responsible for doing any assertions.
pub trait Responder: Send + Sync + 'static {
    fn respond(&self, request: Request) -> Option<Response>;
}

/// Simple responder that returns a general response.
pub struct DefaultResponder;

impl Responder for DefaultResponder {
    fn respond(&self, _: Request) -> Option<Response> {
        Some(Response::default())
    }
}
