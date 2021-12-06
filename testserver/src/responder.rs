use std::{io::Write, thread::sleep, time::Duration};

use crate::{request::Request, response::Response};

/// Provides methods for responding to a request.
pub struct RequestContext<'r> {
    pub(crate) request: &'r mut Request,
    pub(crate) http_request: Option<tiny_http::Request>,
    pub(crate) delay: Option<Duration>,
}

impl<'r> RequestContext<'r> {
    pub(crate) fn new(request: &'r mut Request, http_request: tiny_http::Request) -> Self {
        Self {
            request,
            http_request: Some(http_request),
            delay: None,
        }
    }

    pub fn request(&self) -> &Request {
        self.request
    }

    pub fn send(&mut self, response: Response) {
        if let Some(delay) = self.delay {
            sleep(delay);
        }

        self.http_request
            .take()
            .unwrap()
            .respond(response.into_http_response())
            .unwrap();
    }

    pub fn into_raw(&mut self) -> impl Write {
        self.http_request.take().unwrap().into_writer()
    }

    pub fn set_delay(&mut self, delay: Duration) {
        self.delay = Some(delay);
    }
}

/// A responder is a request-response handler responsible for producing the
/// responses returned by a mock endpoint.
///
/// Responders are not responsible for doing any test assertions.
pub trait Responder: Send + Sync + 'static {
    /// Respond to a request.
    fn respond(&self, ctx: &mut RequestContext<'_>);
}

/// Simple responder that returns a general response.
pub struct DefaultResponder;

impl Responder for DefaultResponder {
    fn respond(&self, ctx: &mut RequestContext<'_>) {
        ctx.send(Response::default());
    }
}
