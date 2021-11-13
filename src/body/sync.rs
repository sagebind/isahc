use super::AsyncBody;
use futures_lite::{future::yield_now, io::AsyncWriteExt};
use sluice::pipe::{pipe, PipeWriter};
use std::{
    borrow::Cow,
    fmt,
    fs::File,
    io::{Cursor, ErrorKind, Read, Result},
};

/// Contains the body of a synchronous HTTP request or response.
///
/// This type is used to encapsulate the underlying stream or region of memory
/// where the contents of the body are stored. A [`Body`] can be created from
/// many types of sources using the [`Into`](std::convert::Into) trait or one of
/// its constructor functions. It can also be created from anything that
/// implements [`Read`], which [`Body`] itself also implements.
///
/// For asynchronous requests, use [`AsyncBody`] instead.
pub struct Body(Inner);

enum Inner {
    Empty,
    Buffer(Cursor<Cow<'static, [u8]>>),
    Reader(Box<dyn Read + Send + Sync>, Option<u64>),
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
        B: AsRef<[u8]> + 'static,
    {
        castaway::match_type!(bytes, {
            Cursor<Cow<'static, [u8]>> as bytes => Self(Inner::Buffer(bytes)),
            Vec<u8> as bytes => Self::from(bytes),
            String as bytes => Self::from(bytes.into_bytes()),
            bytes => Self::from(bytes.as_ref().to_vec()),
        })
    }

    /// Create a streaming body that reads from the given reader.
    ///
    /// The body will have an unknown length. When used as a request body,
    /// [chunked transfer
    /// encoding](https://tools.ietf.org/html/rfc7230#section-4.1) might be used
    /// to send the request.
    pub fn from_reader<R>(reader: R) -> Self
    where
        R: Read + Send + Sync + 'static,
    {
        Self(Inner::Reader(Box::new(reader), None))
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
    pub fn from_reader_sized<R>(reader: R, length: u64) -> Self
    where
        R: Read + Send + Sync + 'static,
    {
        Self(Inner::Reader(Box::new(reader), Some(length)))
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
            _ => false,
        }
    }

    /// Convert this body into an asynchronous one.
    ///
    /// Turning a synchronous operation into an asynchronous one can be quite
    /// the challenge, so this method is used internally only for limited
    /// scenarios in which this can work. If this body is an in-memory buffer,
    /// then the translation is trivial.
    ///
    /// If this body was created from an underlying synchronous reader, then we
    /// create a temporary asynchronous pipe and return a [`Writer`] which will
    /// copy the bytes from the reader to the writing half of the pipe in a
    /// blocking fashion.
    pub(crate) fn into_async(self) -> (AsyncBody, Option<Writer>) {
        match self.0 {
            Inner::Empty => (AsyncBody::empty(), None),
            Inner::Buffer(cursor) => (AsyncBody::from_bytes_static(cursor.into_inner()), None),
            Inner::Reader(reader, len) => {
                let (pipe_reader, writer) = pipe();

                (
                    if let Some(len) = len {
                        AsyncBody::from_reader_sized(pipe_reader, len)
                    } else {
                        AsyncBody::from_reader(pipe_reader)
                    },
                    Some(Writer {
                        reader,
                        writer,
                    }),
                )
            }
        }
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.0 {
            Inner::Empty => Ok(0),
            Inner::Buffer(cursor) => cursor.read(buf),
            Inner::Reader(reader, _) => reader.read(buf),
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

impl From<File> for Body {
    fn from(file: File) -> Self {
        if let Ok(metadata) = file.metadata() {
            Self::from_reader_sized(file, metadata.len())
        } else {
            Self::from_reader(file)
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

/// Helper struct for writing a synchronous reader into an asynchronous pipe.
pub(crate) struct Writer {
    reader: Box<dyn Read + Send + Sync>,
    writer: PipeWriter,
}

impl Writer {
    /// The size of the temporary buffer to use for writing. Larger buffers can
    /// improve performance, but at the cost of more memory.
    ///
    /// Curl's internal buffer size just happens to default to 16 KiB as well,
    /// so this is a natural choice.
    const BUF_SIZE: usize = 16384;

    /// Write the response body from the synchronous reader.
    ///
    /// While this function is async, it isn't a well-behaved one as it blocks
    /// frequently while reading from the request body reader. As long as this
    /// method is invoked in a controlled environment within a thread dedicated
    /// to blocking operations, this is OK.
    pub(crate) async fn write(&mut self) -> Result<()> {
        let mut buf = [0; Self::BUF_SIZE];

        loop {
            let len = match self.reader.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(len) => len,
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    yield_now().await;
                    continue;
                }
                Err(e) => return Err(e),
            };

            self.writer.write_all(&buf[..len]).await?;
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

    #[test]
    fn reader_with_unknown_length() {
        let body = Body::from_reader(std::io::empty());

        assert!(!body.is_empty());
        assert_eq!(body.len(), None);
    }

    #[test]
    fn reader_with_known_length() {
        let body = Body::from_reader_sized(std::io::empty(), 0);

        assert!(!body.is_empty());
        assert_eq!(body.len(), Some(0));
    }

    #[test]
    fn reset_memory_body() {
        let mut body = Body::from("hello world");
        let mut buf = String::new();

        assert_eq!(body.read_to_string(&mut buf).unwrap(), 11);
        assert_eq!(buf, "hello world");
        assert!(body.reset());
        assert_eq!(body.read_to_string(&mut buf).unwrap(), 11);
        assert_eq!(buf, "hello worldhello world");
    }

    #[test]
    fn cannot_reset_reader() {
        let mut body = Body::from_reader(std::io::empty());

        assert_eq!(body.reset(), false);
    }
}
