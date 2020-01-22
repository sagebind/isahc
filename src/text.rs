//! Text decoding routines.

#![cfg(feature = "text-decoding")]

use encoding_rs::{CoderResult, Encoding};
use http::Response;

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
                    log::warn!("unknown encoding '{}', falling back to UTF-8", charset);
                }
            }
        }

        Self::new(encoding_rs::UTF_8)
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
