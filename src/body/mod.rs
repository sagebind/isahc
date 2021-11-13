//! Provides types for working with request and response bodies.

use futures_lite::io::{AsyncRead, BlockOn};
use std::{
    borrow::Cow,
    fmt,
    io::{self, Cursor, Read},
    pin::Pin,
    str,
    task::{Context, Poll},
};

mod sync;

#[allow(unreachable_pub)]
pub use sync::Body;

/// Contains the body of an asynchronous HTTP request or response.
///
/// This type is used to encapsulate the underlying stream or region of memory
/// where the contents of the body are stored. An [`AsyncBody`] can be created
/// from many types of sources using the [`Into`](std::convert::Into) trait or
/// one of its constructor functions.
///
/// For asynchronous requests, you must use an asynchronous body, because the
/// entire request lifecycle is also asynchronous. You can create a body from
/// anything that implements [`AsyncRead`], which [`AsyncBody`] itself also
/// implements.
///
/// For synchronous requests, use [`Body`] instead.
pub struct AsyncBody(Inner);

/// All possible body implementations.
enum Inner {
    /// An empty body.
    Empty,

    /// A body stored in memory.
    Buffer(Cursor<Cow<'static, [u8]>>),

    /// An asynchronous reader.
    Reader(Pin<Box<dyn AsyncRead + Send + Sync>>, Option<u64>),
}

impl AsyncBody {
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
        B: AsRef<[u8]> + 'static,
    {
        castaway::match_type!(bytes, {
            Cursor<Cow<'static, [u8]>> as bytes => Self(Inner::Buffer(bytes)),
            &'static [u8] as bytes => Self::from_static_impl(bytes),
            &'static str as bytes => Self::from_static_impl(bytes.as_bytes()),
            Vec<u8> as bytes => Self::from(bytes),
            String as bytes => Self::from(bytes.into_bytes()),
            bytes => Self::from(bytes.as_ref().to_vec()),
        })
    }

    #[inline]
    fn from_static_impl(bytes: &'static [u8]) -> Self {
        Self(Inner::Buffer(Cursor::new(Cow::Borrowed(bytes))))
    }

    /// Create a streaming body that reads from the given reader.
    ///
    /// The body will have an unknown length. When used as a request body,
    /// [chunked transfer
    /// encoding](https://tools.ietf.org/html/rfc7230#section-4.1) might be used
    /// to send the request.
    pub fn from_reader<R>(read: R) -> Self
    where
        R: AsyncRead + Send + Sync + 'static,
    {
        Self(Inner::Reader(Box::pin(read), None))
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
    pub fn from_reader_sized<R>(read: R, length: u64) -> Self
    where
        R: AsyncRead + Send + Sync + 'static,
    {
        Self(Inner::Reader(Box::pin(read), Some(length)))
    }

    /// Report if this body is empty.
    ///
    /// This is not necessarily the same as checking for `self.len() ==
    /// Some(0)`. Since HTTP message bodies are optional, there is a semantic
    /// difference between the absence of a body and the presence of a
    /// zero-length body. This method will only return `true` for the former.
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
            Inner::Reader(_, len) => *len,
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
            Inner::Reader(_, _) => false,
        }
    }

    /// Turn this asynchronous body into a synchronous one. This is how the
    /// response body is implemented for the synchronous API.
    ///
    /// We do not expose this publicly because while we know that this
    /// implementation works for the bodies _we_ create, it may not work
    /// generally if the underlying reader only supports blocking under a
    /// specific runtime.
    pub(crate) fn into_sync(self) -> sync::Body {
        match self.0 {
            Inner::Empty => sync::Body::empty(),
            Inner::Buffer(cursor) => sync::Body::from_bytes_static(cursor.into_inner()),
            Inner::Reader(reader, Some(len)) => {
                sync::Body::from_reader_sized(BlockOn::new(reader), len)
            }
            Inner::Reader(reader, None) => sync::Body::from_reader(BlockOn::new(reader)),
        }
    }
}

impl AsyncRead for AsyncBody {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match &mut self.0 {
            Inner::Empty => Poll::Ready(Ok(0)),
            Inner::Buffer(cursor) => Poll::Ready(cursor.read(buf)),
            Inner::Reader(read, _) => AsyncRead::poll_read(read.as_mut(), cx, buf),
        }
    }
}

impl Default for AsyncBody {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<()> for AsyncBody {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}

impl From<Vec<u8>> for AsyncBody {
    fn from(body: Vec<u8>) -> Self {
        Self(Inner::Buffer(Cursor::new(Cow::Owned(body))))
    }
}

impl From<&'_ [u8]> for AsyncBody {
    fn from(body: &[u8]) -> Self {
        body.to_vec().into()
    }
}

impl From<String> for AsyncBody {
    fn from(body: String) -> Self {
        body.into_bytes().into()
    }
}

impl From<&'_ str> for AsyncBody {
    fn from(body: &str) -> Self {
        body.as_bytes().into()
    }
}

impl<T: Into<Self>> From<Option<T>> for AsyncBody {
    fn from(body: Option<T>) -> Self {
        match body {
            Some(body) => body.into(),
            None => Self::empty(),
        }
    }
}

impl fmt::Debug for AsyncBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.len() {
            Some(len) => write!(f, "AsyncBody({})", len),
            None => write!(f, "AsyncBody(?)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::{
        future::{block_on, zip},
        io::AsyncReadExt,
    };

    static_assertions::assert_impl_all!(AsyncBody: Send, Sync);

    #[test]
    fn empty_body() {
        let body = AsyncBody::empty();

        assert!(body.is_empty());
        assert_eq!(body.len(), Some(0));
    }

    #[test]
    fn zero_length_body() {
        let body = AsyncBody::from(vec![]);

        assert!(!body.is_empty());
        assert_eq!(body.len(), Some(0));
    }

    #[test]
    fn reader_with_unknown_length() {
        let body = AsyncBody::from_reader(futures_lite::io::empty());

        assert!(!body.is_empty());
        assert_eq!(body.len(), None);
    }

    #[test]
    fn reader_with_known_length() {
        let body = AsyncBody::from_reader_sized(futures_lite::io::empty(), 0);

        assert!(!body.is_empty());
        assert_eq!(body.len(), Some(0));
    }

    #[test]
    fn reset_memory_body() {
        block_on(async {
            let mut body = AsyncBody::from("hello world");
            let mut buf = String::new();

            assert_eq!(body.read_to_string(&mut buf).await.unwrap(), 11);
            assert_eq!(buf, "hello world");
            assert!(body.reset());
            buf.clear(); // read_to_string panics if the destination isn't empty
            assert_eq!(body.read_to_string(&mut buf).await.unwrap(), 11);
            assert_eq!(buf, "hello world");
        });
    }

    #[test]
    fn cannot_reset_reader() {
        let mut body = AsyncBody::from_reader(futures_lite::io::empty());

        assert_eq!(body.reset(), false);
    }

    #[test]
    fn sync_memory_into_async() {
        let (body, writer) = Body::from("hello world").into_async();

        assert!(writer.is_none());
        assert_eq!(body.len(), Some(11));
    }

    #[test]
    fn sync_reader_into_async() {
        block_on(async {
            let (mut body, writer) = Body::from_reader("hello world".as_bytes()).into_async();

            assert!(writer.is_some());

            // Write from the writer concurrently as we read from the body.
            zip(
                async move {
                    writer.unwrap().write().await.unwrap();
                },
                async move {
                    let mut buf = String::new();
                    body.read_to_string(&mut buf).await.unwrap();
                    assert_eq!(buf, "hello world");
                },
            )
            .await;
        });
    }
}
