//! Provides types for working with request and response bodies.

use futures_lite::{future::block_on, io::{AsyncRead, AsyncReadExt}};
use std::{
    borrow::Cow,
    fmt,
    io::{self, Cursor, Read},
    pin::Pin,
    str,
    task::{Context, Poll},
};

pub mod sync;

/// Contains the body of an HTTP request or response.
///
/// This type is used to encapsulate the underlying stream or region of memory
/// where the contents of the body are stored. A `Body` can be created from many
/// types of sources using the [`Into`](std::convert::Into) trait or one of its
/// constructor functions.
///
/// Since the entire request life-cycle in Isahc is asynchronous, bodies must
/// also be asynchronous. You can create a body from anything that implements
/// [`AsyncRead`], which [`Body`] itself also implements.
pub struct Body(Inner);

/// All possible body implementations.
enum Inner {
    /// An empty body.
    Empty,

    /// A body stored in memory.
    Buffer(Cursor<Cow<'static, [u8]>>),

    /// An asynchronous reader.
    AsyncRead(Pin<Box<dyn AsyncRead + Send + Sync>>, Option<u64>),
}

impl Body {
    /// Create a new empty body.
    ///
    /// An empty body represents the *absence* of a body, which is semantically
    /// different than the presence of a body of zero length.
    pub const fn empty() -> Self {
        Self(Inner::Empty)
    }

    /// Create a new body from a potentially static byte buffer.
    ///
    /// The body will have a known length equal to the number of bytes given.
    ///
    /// This will try to prevent a copy if the type passed in can be re-used,
    /// otherwise the buffer will be copied first. This method guarantees to not
    /// require a copy for the following types:
    ///
    /// - `&'static [u8]`
    /// - `&'static str`
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::Body;
    ///
    /// // Create a body from a static string.
    /// let body = Body::from_bytes_static("hello world");
    /// ```
    #[inline]
    pub fn from_bytes_static<B>(bytes: B) -> Self
    where
        B: AsRef<[u8]> + 'static
    {
        match_type! {
            <bytes as Cursor<Cow<'static, [u8]>>> => Self(Inner::Buffer(bytes)),
            <bytes as &'static [u8]> => Self::from_static_impl(bytes),
            <bytes as &'static str> => Self::from_static_impl(bytes.as_bytes()),
            <bytes as Vec<u8>> => Self::from(bytes),
            <bytes as String> => Self::from(bytes.into_bytes()),
            bytes => Self::from(bytes.as_ref().to_vec()),
        }
    }

    #[inline]
    fn from_static_impl(bytes: &'static [u8]) -> Self {
        Self(Inner::Buffer(Cursor::new(Cow::Borrowed(bytes))))
    }

    /// Create a streaming body that reads from the given reader.
    ///
    /// The body will have an unknown length. When used as a request body,
    /// chunked transfer encoding might be used to send the request.
    pub fn from_reader(read: impl AsyncRead + Send + Sync + 'static) -> Self {
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
    pub fn from_reader_sized(read: impl AsyncRead + Send + Sync + 'static, length: u64) -> Self {
        Body(Inner::AsyncRead(Box::pin(read), Some(length)))
    }

    /// Report if this body is empty.
    pub fn is_empty(&self) -> bool {
        match self.0 {
            Inner::Empty => true,
            _ => false,
        }
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
    pub fn len(&self) -> Option<u64> {
        match &self.0 {
            Inner::Empty => Some(0),
            Inner::Buffer(bytes) => Some(bytes.get_ref().len() as u64),
            Inner::AsyncRead(_, len) => *len,
        }
    }

    /// If this body is repeatable, reset the body stream back to the start of
    /// the content. Returns `false` if the body cannot be reset.
    pub fn reset(&mut self) -> bool {
        match &mut self.0 {
            Inner::Empty => true,
            Inner::Buffer(cursor) => {
                cursor.set_position(0);
                true
            }
            Inner::AsyncRead(_, _) => false,
        }
    }

    pub(crate) fn into_sync(self) -> sync::Body {
        struct BlockingReader<R>(R);

        impl<R: AsyncRead + Unpin> Read for BlockingReader<R> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                block_on(self.0.read(buf))
            }
        }

        match self.0 {
            Inner::Empty => sync::Body::from_bytes_static(b""),
            Inner::Buffer(cursor) => sync::Body::from_bytes_static(cursor.into_inner()),
            Inner::AsyncRead(reader, Some(len)) => {
                sync::Body::from_reader_sized(BlockingReader(reader), len)
            },
            Inner::AsyncRead(reader, None) => {
                sync::Body::from_reader(BlockingReader(reader))
            },
        }
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            Inner::Empty => Ok(0),
            Inner::Buffer(cursor) => cursor.read(buf),
            Inner::AsyncRead(reader, _) => block_on(reader.read(buf)),
        }
    }
}

impl AsyncRead for Body {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match &mut self.0 {
            Inner::Empty => Poll::Ready(Ok(0)),
            Inner::Buffer(cursor) => Poll::Ready(cursor.read(buf)),
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
        Self(Inner::Buffer(Cursor::new(Cow::Owned(body))))
    }
}

impl From<&'_ [u8]> for Body {
    fn from(body: &[u8]) -> Self {
        body.to_vec().into()
    }
}

impl From<String> for Body {
    fn from(body: String) -> Self {
        body.into_bytes().into()
    }
}

impl From<&'_ str> for Body {
    fn from(body: &str) -> Self {
        body.as_bytes().into()
    }
}

impl<T: Into<Body>> From<Option<T>> for Body {
    fn from(body: Option<T>) -> Self {
        match body {
            Some(body) => body.into(),
            None => Self::empty(),
        }
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.len() {
            Some(len) => write!(f, "Body({})", len),
            None => write!(f, "Body(?)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(Body: Send, Sync);

    #[test]
    fn empty_body() {
        let body = Body::empty();

        assert!(body.is_empty());
        assert_eq!(body.len(), Some(0));
    }

    #[test]
    fn zero_length_body() {
        let body = Body::from(vec![]);

        assert!(!body.is_empty());
        assert_eq!(body.len(), Some(0));
    }
}
