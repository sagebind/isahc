use crate::{
    request::MockRequest,
    response::MockResponse,
};

pub trait Responder: Send + Sync + 'static {
    fn respond(&self, request: MockRequest) -> Option<MockResponse>;
}

pub struct DefaultResponder;

impl Responder for DefaultResponder {
    fn respond(&self, _: MockRequest) -> Option<MockResponse> {
        Some(MockResponse::default())
    }
}
