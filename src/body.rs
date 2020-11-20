//! Provides types for working with request and response bodies.

use bytes::Bytes;
use futures_lite::{future::block_on, io::{AsyncRead, AsyncReadExt}};
use std::{
    fmt,
    io::{self, Cursor, Read},
    pin::Pin,
    str,
    task::{Context, Poll},
};

macro_rules! match_type {
    {
        $(
            <$name:ident as $T:ty> => $branch:expr,
        )*
        $defaultName:ident => $defaultBranch:expr,
    } => {{
        match () {
            $(
                _ if ::std::any::Any::type_id(&$name) == ::std::any::TypeId::of::<$T>() => {
                    #[allow(unsafe_code)]
                    let $name: $T = unsafe {
                        ::std::mem::transmute_copy::<_, $T>(&::std::mem::ManuallyDrop::new($name))
                    };
                    $branch
                }
            )*
            _ => $defaultBranch,
        }
    }};
}

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
    Bytes(Cursor<Bytes>),

    /// An asynchronous reader.
    AsyncRead(Pin<Box<dyn AsyncRead + Send + Sync>>, Option<u64>),
}

impl Body {
    /// Create a new empty body.
    ///
    /// An empty body represents the *absence* of a body, which is semantically
    /// different than the presence of a body of zero length.
    pub const fn empty() -> Self {
        Body(Inner::Empty)
    }

    /// Create a new body from bytes stored in memory.
    ///
    /// The body will have a known length equal to the number of bytes given.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::Body;
    ///
    /// // Create a body from a string.
    /// let body = Body::from_bytes("hello world");
    /// ```
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self {
        bytes.as_ref().to_vec().into()
    }

    /// Attempt to create a new body from a shared [`Bytes`] buffer.
    ///
    /// This will try to prevent a copy if the type passed is the type used
    /// internally, and will copy the data if it is not.
    pub fn from_maybe_shared(bytes: impl AsRef<[u8]> + 'static) -> Self {
        Body(Inner::Bytes(Cursor::new(match_type! {
            <bytes as Bytes> => bytes,
            <bytes as &'static [u8]> => Bytes::from_static(bytes),
            bytes => bytes.as_ref().to_vec().into(),
        })))
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
            Inner::Bytes(bytes) => Some(bytes.get_ref().len() as u64),
            Inner::AsyncRead(_, len) => *len,
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
            }
            Inner::AsyncRead(_, _) => false,
        }
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            Inner::Empty => Ok(0),
            Inner::Bytes(cursor) => cursor.read(buf),
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
            Inner::Bytes(cursor) => Poll::Ready(cursor.read(buf)),
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
        Self::from_maybe_shared(Bytes::from(body))
    }
}

impl From<&'static [u8]> for Body {
    fn from(body: &'static [u8]) -> Self {
        Self::from_maybe_shared(Bytes::from(body))
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
        let body = Body::from_bytes(vec![]);

        assert!(!body.is_empty());
        assert_eq!(body.len(), Some(0));
    }
}
