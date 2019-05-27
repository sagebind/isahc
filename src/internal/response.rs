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
            headers: http::HeaderMap::new(),
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

/// Producing end of a response future that builds up the response object
/// incrementally.
pub struct ResponseProducer {
    sender: Option<Sender<Response<Body>>>,

    /// Status code of the response.
    pub(crate) status_code: Option<http::StatusCode>,

    /// HTTP version of the response.
    pub(crate) version: Option<http::Version>,

    /// Response headers received so far.
    pub(crate) headers: http::HeaderMap,
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

    /// Finishes constructing the response and sends it to the receiver.
    pub fn finish(&mut self, body: Body) -> bool {
        let mut builder = http::Response::builder();
        builder.status(self.status_code.take().unwrap());
        builder.version(self.version.take().unwrap());

        for (name, values) in self.headers.drain() {
            for value in values {
                builder.header(&name, value);
            }
        }

        let response = builder
            .body(body)
            .unwrap();

        match self.sender.take() {
            Some(sender) => match sender.send(response) {
                Ok(()) => true,
                Err(_) => {
                    log::info!("response future cancelled");
                    false
                },
            }
            None => {
                log::warn!("response future already completed!");
                false
            },
        }
    }
}

static_assertions::assert_impl!(f; ResponseFuture, Send);
