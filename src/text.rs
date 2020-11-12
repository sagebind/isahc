//! Text decoding routines.

#![cfg(feature = "text-decoding")]

use encoding_rs::{CoderResult, Encoding};
use futures_lite::io::{AsyncRead, AsyncReadExt};
use http::Response;
use std::{
    future::Future,
    io,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

// This macro abstracts over async and sync decoding, since the implementation
// of decoding a stream into text is the same.
macro_rules! decode_reader {
    ($decoder:expr, $buf:ident, $read:expr) => {{
        let mut decoder = $decoder;
        let mut buf = [0; 8192];
        let mut unread = 0;

        loop {
            let $buf = &mut buf[unread..];
            let len = match $read {
                Ok(0) => break,
                Ok(len) => len,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };

            unread = decoder.push(&buf[..unread + len]).len();
        }

        Ok(decoder.finish(&buf[..unread]))
    }};
}

/// A future returning a response body decoded as text.
#[allow(missing_debug_implementations)]
pub struct TextFuture<'a, R> {
    inner: Pin<Box<dyn Future<Output = io::Result<String>> + 'a>>,
    _phantom: PhantomData<R>,
}

impl<'a, R: Unpin> Future for TextFuture<'a, R> {
    type Output = io::Result<String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.as_mut().inner.as_mut().poll(cx)
    }
}

// Since we are boxing our future, we can't conditionally implement `Send` based
// on whether the original future is `Send`. However, we know after inspection
// that everything inside our implementation is `Send` except for the reader,
// which may or may not be. We then put the reader in our wrapper future type
// and conditionally implement `Send` if the reader is also `Send`.
#[allow(unsafe_code)]
unsafe impl<'r, R: Send> Send for TextFuture<'r, R> {}

/// A streaming text decoder that supports multiple encodings.
pub(crate) struct Decoder {
    /// Inner decoder implementation.
    decoder: encoding_rs::Decoder,

    /// The output string that characters are accumulated to.
    output: String,
}

impl Decoder {
    /// Create a new decoder with the given encoding.
    pub(crate) fn new(encoding: &'static Encoding) -> Self {
        Self {
            decoder: encoding.new_decoder(),
            output: String::new(),
        }
    }

    /// Create a new encoder suitable for decoding the given response.
    pub(crate) fn for_response<T>(response: &Response<T>) -> Self {
        if let Some(content_type) = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.parse::<mime::Mime>().ok())
        {
            if let Some(charset) = content_type.get_param(mime::CHARSET) {
                if let Some(encoding) =
                    encoding_rs::Encoding::for_label(charset.as_str().as_bytes())
                {
                    return Self::new(encoding);
                } else {
                    tracing::warn!("unknown encoding '{}', falling back to UTF-8", charset);
                }
            }
        }

        Self::new(encoding_rs::UTF_8)
    }

    /// Consume this decoder to decode text from a given synchronous reader.
    pub(crate) fn decode_reader(self, mut reader: impl io::Read) -> io::Result<String> {
        decode_reader!(self, buf, reader.read(buf))
    }

    /// Consume this decoder to decode text from a given asynchronous reader.
    pub(crate) fn decode_reader_async<'r, R>(self, mut reader: R) -> TextFuture<'r, R>
    where
        R: AsyncRead + Unpin + 'r,
    {
        TextFuture {
            inner: Box::pin(async move {
                decode_reader!(self, buf, reader.read(buf).await)
            }),
            _phantom: PhantomData,
        }
    }

    /// Push additional bytes into the decoder, returning any trailing bytes
    /// that formed a partial character.
    pub(crate) fn push<'b>(&mut self, buf: &'b [u8]) -> &'b [u8] {
        self.decode(buf, false)
    }

    /// Mark the stream as complete and finish the decoding process, returning
    /// the resulting string.
    pub(crate) fn finish(mut self, buf: &[u8]) -> String {
        self.decode(buf, true);
        self.output
    }

    fn decode<'b>(&mut self, mut buf: &'b [u8], last: bool) -> &'b [u8] {
        loop {
            let (result, consumed, _) = self.decoder.decode_to_string(buf, &mut self.output, last);
            buf = &buf[consumed..];

            match result {
                CoderResult::InputEmpty => break,
                CoderResult::OutputFull => self
                    .output
                    .reserve(self.decoder.max_utf8_buffer_length(buf.len()).unwrap()),
            }
        }

        // If last is true, buf should always be fully consumed.
        if cfg!(debug) && last {
            assert_eq!(buf.len(), 0);
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Body;

    static_assertions::assert_impl_all!(TextFuture<'_, &mut Body>: Send);

    #[test]
    fn utf8_decode() {
        let mut decoder = Decoder::new(encoding_rs::UTF_8);

        assert_eq!(decoder.push(b"hello"), &[]);
        assert_eq!(decoder.push(b" "), &[]);
        assert_eq!(decoder.finish(b"world"), "hello world");
    }

    #[test]
    fn utf16_decode() {
        let bytes = encoding_rs::UTF_16BE.encode("hello world!").0.into_owned();
        let mut decoder = Decoder::new(encoding_rs::UTF_8);

        for byte in bytes {
            decoder.push(&[byte]);
        }

        assert_eq!(decoder.finish(&[]), "hello world!");
    }
}
