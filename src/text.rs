//! Text decoding routines.

use encoding_rs::{CoderResult, Encoding};

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
