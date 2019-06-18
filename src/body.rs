//! Provides types for working with request and response bodies.

use crate::Error;
use bytes::Bytes;
use futures::io::{AsyncRead, AsyncReadExt};
use std::fmt;
use std::io::{self, Cursor, Read};
use std::task::*;
use std::pin::Pin;
use std::str;

/// Contains the body of an HTTP request or response.
///
/// This type is used to encapsulate the underlying stream or region of memory
/// where the contents of the body are stored. A `Body` can be created from many
/// types of sources using the [`Into`](std::convert::Into) trait or one of its
/// constructor functions.
///
/// Since the entire request life-cycle in cHTTP is asynchronous, bodies must
/// also be asynchronous. You can create a body from anything that implements
/// [`AsyncRead`](futures::io::AsyncRead), which `Body` itself also implements.
pub struct Body(Inner);

/// All possible body implementations.
enum Inner {
    /// An empty body.
    Empty,

    /// A body stored in memory.
    Bytes(Cursor<Bytes>),

    /// An asynchronous reader.
    AsyncRead(Pin<Box<dyn AsyncRead + Send>>, Option<usize>),
}

impl Body {
    /// Create a new empty body.
    ///
    /// An empty body will have a known length of 0 bytes.
    pub const fn empty() -> Self {
        Body(Inner::Empty)
    }

    /// Create a new body from bytes stored in memory.
    ///
    /// The body will have a known length equal to the number of bytes given.
    pub fn bytes(bytes: impl Into<Bytes>) -> Self {
        Body(Inner::Bytes(Cursor::new(bytes.into())))
    }

    /// Create a streaming body that reads from the given reader.
    ///
    /// The body will have an unknown length. When used as a request body,
    /// chunked transfer encoding might be used to send the request.
    pub fn reader(read: impl AsyncRead + Send + 'static) -> Self {
        Body(Inner::AsyncRead(Box::pin(read), None))
    }

    /// Create a streaming body with a known length.
    ///
    /// If the size of the body is known in advance, such as with a file, then
    /// this function can be used to create a body that can determine its
    /// `Content-Length` while still reading the bytes asynchronously.
    ///
    /// Giving a value for `length` that doesn't actually match how much data
    /// the reader will produce may result in errors when sending the body in a
    /// request.
    pub fn reader_sized(read: impl AsyncRead + Send + 'static, length: usize) -> Self {
        Body(Inner::AsyncRead(Box::pin(read), Some(length)))
    }

    /// Report if this body is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }

    /// Get the size of the body, if known.
    ///
    /// The value reported by this method is used to set the `Content-Length`
    /// for outgoing requests.
    ///
    /// When coming from a response, this method will report the value of the
    /// `Content-Length` response header if present. If this method returns
    /// `None` then there's a good chance that the server used something like
    /// chunked transfer encoding to send the response body.
    ///
    /// Since the length may be determined totally separately from the actual
    /// bytes, even if a value is returned it should not be relied on as always
    /// being accurate, and should be treated as a "hint".
    pub fn len(&self) -> Option<usize> {
        match &self.0 {
            Inner::Empty => Some(0),
            Inner::Bytes(bytes) => Some(bytes.get_ref().len()),
            Inner::AsyncRead(_, len) => len.clone(),
        }
    }

    /// If this body is repeatable, reset the body stream back to the start of
    /// the content. Returns `false` if the body cannot be reset.
    pub fn reset(&mut self) -> bool {
        match &mut self.0 {
            Inner::Empty => true,
            Inner::Bytes(cursor) => {
                cursor.set_position(0);
                true
            },
            Inner::AsyncRead(_, _) => false,
        }
    }

    /// Get the response body as a string.
    ///
    /// If the body comes from a stream, the steam bytes will be consumed and
    /// this method will return an empty string next call. If this body supports
    /// seeking, you can seek to the beginning of the body if you need to call
    /// this method again later.
    pub async fn text(&mut self) -> Result<String, Error> {
        if self.is_empty() {
            Ok(String::new())
        } else {
            let mut bytes = Vec::new();
            AsyncReadExt::read_to_end(self, &mut bytes).await?;
            Ok(String::from_utf8(bytes)?)
        }
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        futures::executor::block_on(AsyncReadExt::read(self, buf))
    }
}

impl AsyncRead for Body {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        match &mut self.0 {
            Inner::Empty => Poll::Ready(Ok(0)),
            Inner::Bytes(cursor) => AsyncRead::poll_read(Pin::new(cursor), cx, buf),
            Inner::AsyncRead(read, _) => AsyncRead::poll_read(read.as_mut(), cx, buf),
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<()> for Body {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}

impl From<Vec<u8>> for Body {
    fn from(body: Vec<u8>) -> Self {
        Self::bytes(body)
    }
}

impl From<&'static [u8]> for Body {
    fn from(body: &'static [u8]) -> Self {
        Bytes::from_static(body).into()
    }
}

impl From<Bytes> for Body {
    fn from(body: Bytes) -> Self {
        Self::bytes(body)
    }
}

impl From<String> for Body {
    fn from(body: String) -> Self {
        body.into_bytes().into()
    }
}

impl From<&'static str> for Body {
    fn from(body: &'static str) -> Self {
        body.as_bytes().into()
    }
}

impl<T: Into<Body>> From<Option<T>> for Body {
    fn from(body: Option<T>) -> Self {
        match body {
            Some(body) => body.into(),
            None => Self::default(),
        }
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.len() {
            Some(len) => write!(f, "Body({})", len),
            None => write!(f, "Body(?)"),
        }
    }
}

static_assertions::assert_impl!(body; Body, Send);
