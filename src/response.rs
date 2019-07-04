use crate::Error;
use futures::future::FutureExt;
use futures::io::{AsyncRead, AsyncReadExt};
use http::Response;
use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::task::*;

/// Provides extension methods for working with HTTP responses.
pub trait ResponseExt<T> {
    /// Get the response body as a string.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
    fn text(&mut self) -> Result<String, Error> where T: Read;

    /// Get the response body as a string asynchronously.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
    fn text_async<'r>(&'r mut self) -> TextFuture<'r, T> where T: AsyncRead + Unpin;
}

impl<T> ResponseExt<T> for Response<T> {
    fn text(&mut self) -> Result<String, Error> where T: Read {
        let mut s = String::default();
        self.body_mut().read_to_string(&mut s)?;
        Ok(s)
    }

    fn text_async(&mut self) -> TextFuture<'_, T> where T: AsyncRead + Unpin {
        TextFuture::new(self.body_mut())
    }
}

/// A future that produces a string from an [`AsyncRead`] reader.
pub struct TextFuture<'r, R: Unpin> {
    buffer: Option<Box<Vec<u8>>>,
    inner: Option<futures::io::ReadToEnd<'r, R>>,
}

impl<'r, R: AsyncRead + Unpin> TextFuture<'r, R> {
    /// Create a new future from a given reader.
    fn new(reader: &'r mut R) -> Self {
        // We can't split the borrow on the buffer safely, so we heap-allocate
        // it and pretend that it has the lifetime 'r, carefully making sure
        // that we remove references to the buffer first before cleaning it up.
        let mut buffer = Box::new(Vec::new());
        let ptr: *mut Vec<u8> = &mut *buffer as *mut _;
        let buffer_ref = unsafe { ptr.as_mut().unwrap() };

        Self {
            buffer: Some(buffer),
            inner: Some(AsyncReadExt::read_to_end(reader, buffer_ref)),
        }
    }
}

impl<'r, R: AsyncRead + Unpin> Future for TextFuture<'r, R> {
    type Output = Result<String, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.as_mut().unwrap().poll_unpin(cx) {
            // Buffer isn't full yet.
            Poll::Pending => Poll::Pending,

            // Read error
            Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),

            // Buffer has been filled, try to parse as UTF-8
            Poll::Ready(Ok(())) => match String::from_utf8(*self.buffer.take().unwrap()) {
                Ok(string) => Poll::Ready(Ok(string)),
                Err(e) => Poll::Ready(Err(e.into())),
            }
        }
    }
}

impl<'r, R: Unpin> Drop for TextFuture<'r, R> {
    fn drop(&mut self) {
        // Make sure we drop the inner future before the buffer, since it thinks
        // the buffer has a lifetime of 'r.
        self.inner.take();
    }
}
