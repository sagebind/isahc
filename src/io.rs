use futures_io::AsyncRead;
use futures_util::future::FutureExt;
use futures_util::io::{AsyncReadExt, ReadToEnd};
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::task::{Context, Poll};

/// A future that produces a string from an [`AsyncRead`] reader.
#[derive(Debug)]
pub struct Text<'r, R: Unpin> {
    #[allow(clippy::box_vec)]
    buffer: Option<Box<Vec<u8>>>,
    inner: Option<ReadToEnd<'r, R>>,
}

impl<'r, R: AsyncRead + Unpin> Text<'r, R> {
    /// Create a new future from a given reader.
    #[allow(unsafe_code)]
    pub(crate) fn new(reader: &'r mut R) -> Self {
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

impl<'r, R: AsyncRead + Unpin> Future for Text<'r, R> {
    type Output = Result<String, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.as_mut().unwrap().poll_unpin(cx) {
            // Buffer isn't full yet.
            Poll::Pending => Poll::Pending,

            // Read error
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),

            // Buffer has been filled, try to parse as UTF-8
            Poll::Ready(Ok(())) => match String::from_utf8(*self.buffer.take().unwrap()) {
                Ok(string) => Poll::Ready(Ok(string)),
                Err(_) => Poll::Ready(Err(Error::new(
                    ErrorKind::InvalidData,
                    "stream did not contain valid UTF-8",
                ))),
            },
        }
    }
}

impl<'r, R: Unpin> Drop for Text<'r, R> {
    fn drop(&mut self) {
        // Make sure we drop the inner future before the buffer, since it thinks
        // the buffer has a lifetime of 'r.
        self.inner.take();
    }
}
