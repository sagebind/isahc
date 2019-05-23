//! Provides types for working with request and response bodies.

use crate::error::Error;
use bytes::Bytes;
use futures::prelude::*;
use std::fmt;
use std::io::{self, Cursor, Read, SeekFrom};
use std::task::*;
use std::pin::Pin;
use std::str;

/// Contains the body of an HTTP request or response.
///
/// This type is used to encapsulate the underlying stream or region of memory where the contents of the body is stored.
/// A `Body` can be created from many types of sources using the [`Into`](std::convert::Into) trait.
pub struct Body(Inner);

/// All possible body implementations.
enum Inner {
    /// An empty body.
    Empty,
    /// An asynchronous reader.
    AsyncRead {
        object: Pin<Box<dyn AsyncRead + Send>>,
        size_hint: Option<usize>,
    },
    /// An asynchronous reader that can also seek.
    AsyncReadSeek {
        object: Pin<Box<dyn AsyncReadSeek + Send>>,
        size_hint: Option<usize>,
    },
}

impl Body {
    pub const EMPTY: Self = Body(Inner::Empty);

    pub fn from_read(read: impl AsyncRead + Send + 'static) -> Self {
        Body(Inner::AsyncRead {
            object: Box::pin(read),
            size_hint: None,
        })
    }

    /// Report if this body is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }

    /// Get the size of the body, if known.
    pub fn len(&self) -> Option<usize> {
        match &self.0 {
            Inner::Empty => Some(0),
            Inner::AsyncRead {size_hint, ..} => size_hint.clone(),
            Inner::AsyncReadSeek {size_hint, ..} => size_hint.clone(),
        }
    }

    /// If this body is repeatable, reset the body stream back to the start of
    /// the content. Returns `false` if the body cannot be reset.
    pub async fn reset(&mut self) -> bool {
        match &mut self.0 {
            Inner::Empty => true,
            Inner::AsyncReadSeek {object, ..} => AsyncSeekExt::seek(&mut *object, SeekFrom::Start(0)).await.is_ok(),
            _ => false,
        }
    }

    /// Get the response body as a string.
    ///
    /// If the body comes from a stream, the steam bytes will be consumed and this method will return an empty string
    /// next call. If this body supports seeking, you can seek to the beginning of the body if you need to call this
    /// method again later.
    pub async fn text(&mut self) -> Result<String, Error> {
        if self.is_empty() {
            Ok(String::new())
        } else {
            let mut bytes = Vec::new();
            AsyncReadExt::read_to_end(self, &mut bytes).await?;
            Ok(String::from_utf8(bytes)?)
        }
    }

    /// Attempt to parse the response as JSON.
    #[cfg(feature = "json")]
    pub async fn json(&mut self) -> Result<json::JsonValue, Error> {
        let text = self.text().await?;
        Ok(json::parse(&text)?)
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        futures::executor::block_on(AsyncReadExt::read(self, buf))
    }
}

impl AsyncRead for Body {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        match &mut self.0 {
            Inner::Empty => Poll::Ready(Ok(0)),
            Inner::AsyncRead {object, ..} => AsyncRead::poll_read(object.as_mut(), cx, buf),
            Inner::AsyncReadSeek {object, ..} => AsyncRead::poll_read(object.as_mut(), cx, buf),
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Body::EMPTY
    }
}

impl From<Vec<u8>> for Body {
    fn from(body: Vec<u8>) -> Self {
        Body(Inner::AsyncReadSeek {
            object: Box::pin(Cursor::new(body)),
            size_hint: Some(body.len()),
        })
    }
}

impl From<&'static [u8]> for Body {
    fn from(body: &'static [u8]) -> Self {
        Body(Inner::AsyncReadSeek {
            object: Box::pin(Cursor::new(body)),
            size_hint: Some(body.len()),
        })
    }
}

impl From<Bytes> for Body {
    fn from(body: Bytes) -> Self {
        Body(Inner::AsyncReadSeek {
            object: Box::pin(Cursor::new(body)),
            size_hint: Some(body.len()),
        })
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

// impl From<File> for Body {
//     fn from(body: File) -> Self {
//         Self::from_reader(body)
//     }
// }

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

/// Helper trait combining `AsyncRead` and `AsyncSeek`.
trait AsyncReadSeek: AsyncRead + AsyncSeek {}

impl<T> AsyncReadSeek for T where T: AsyncRead + AsyncSeek {}
