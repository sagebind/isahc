use crate::body::Body;
use crate::error::Error;
use futures::channel::oneshot::*;
use futures::prelude::*;
use http::Response;
use std::pin::Pin;
use std::task::*;

// A future for a response.
pub struct ResponseFuture {
    receiver: Receiver<Response<Body>>,
}

impl ResponseFuture {
    pub fn new() -> (Self, ResponseProducer) {
        let (sender, receiver) = channel();

        let future = Self {
            receiver,
        };

        let producer = ResponseProducer {
            sender: Some(sender),
            status_code: None,
            version: None,
            headers: None,
        };

        (future, producer)
    }
}

impl Future for ResponseFuture {
    type Output = Result<Response<Body>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let inner = Pin::new(&mut self.receiver);

        match inner.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(response)) => Poll::Ready(Ok(response)),
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::Canceled)),
        }
    }
}

pub struct ResponseProducer {
    sender: Option<Sender<Response<Body>>>,

    /// Status code of the response.
    status_code: Option<http::StatusCode>,

    /// HTTP version of the response.
    version: Option<http::Version>,

    /// Response headers received so far.
    headers: Option<http::HeaderMap>,
}

impl ResponseProducer {
    pub fn is_canceled(&self) -> bool {
        match self.sender.as_ref() {
            Some(sender) => sender.is_canceled(),
            None => false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.sender.is_none()
    }

    pub fn is_closed(&self) -> bool {
        match self.sender.as_ref() {
            Some(sender) => sender.is_canceled(),
            None => true,
        }
    }

    pub fn finish(&mut self, body: Body) -> Result<(), Body> {
        Err(body)
    }
}

static_assertions::assert_impl!(f; ResponseFuture, Send);
