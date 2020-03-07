use crate::Metrics;
use http::{Response, Uri};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

/// Provides extension methods for working with HTTP responses.
pub trait ResponseExt<T> {
    /// Get the effective URI of this response. This value differs from the
    /// original URI provided when making the request if at least one redirect
    /// was followed.
    ///
    /// This information is only available if populated by the HTTP client that
    /// produced the response.
    fn effective_uri(&self) -> Option<&Uri>;

    /// If request metrics are enabled for this particular transfer, return a
    /// metrics object containing a live view of currently available data.
    ///
    /// By default metrics are disabled and `None` will be returned. To enable
    /// metrics you can use
    /// [`Configurable::metrics`](crate::config::Configurable::metrics).
    fn metrics(&self) -> Option<&Metrics>;

    /// Copy the response body into a writer.
    ///
    /// Returns the number of bytes that were written.
    fn copy_to(&mut self, writer: impl Write) -> io::Result<u64>
    where
        T: Read;

    /// Write the response body to a file.
    ///
    /// This method makes it convenient to download a file using a GET request
    /// and write it to a file synchronously in a single chain of calls.
    ///
    /// Returns the number of bytes that were written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// isahc::get("https://httpbin.org/image/jpeg")?
    ///     .copy_to_file("myimage.jpg")?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn copy_to_file(&mut self, path: impl AsRef<Path>) -> io::Result<u64>
    where
        T: Read,
    {
        File::create(path).and_then(|f| self.copy_to(f))
    }

    /// Read the response body as a string.
    ///
    /// The encoding used to decode the response body into a string depends on
    /// the response. If the body begins with a [Byte Order Mark
    /// (BOM)](https://en.wikipedia.org/wiki/Byte_order_mark), then UTF-8,
    /// UTF-16LE or UTF-16BE is used as indicated by the BOM. If no BOM is
    /// present, the encoding specified in the `charset` parameter of the
    /// `Content-Type` header is used if present. Otherwise UTF-8 is assumed.
    ///
    /// If the response body contains any malformed characters or characters not
    /// representable in UTF-8, the offending bytes will be replaced with
    /// `U+FFFD REPLACEMENT CHARACTER`, which looks like this: �.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once.
    ///
    /// # Availability
    ///
    /// This method is only available when the
    /// [`text-decoding`](index.html#text-decoding) feature is enabled, which it
    /// is by default.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let text = isahc::get("https://example.org")?.text()?;
    /// println!("{}", text);
    /// # Ok::<(), isahc::Error>(())
    /// ```
    #[cfg(feature = "text-decoding")]
    fn text(&mut self) -> io::Result<String>
    where
        T: Read;

    /// Read the response body as a string asynchronously.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once.
    ///
    /// # Availability
    ///
    /// This method is only available when the
    /// [`text-decoding`](index.html#text-decoding) feature is enabled, which it
    /// is by default.
    #[cfg(feature = "text-decoding")]
    fn text_async(&mut self) -> crate::text::TextFuture<'_, &mut T>
    where
        T: futures_io::AsyncRead + Unpin;

    /// Deserialize the response body as JSON into a given type.
    ///
    /// # Availability
    ///
    /// This method is only available when the [`json`](index.html#json) feature
    /// is enabled.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    /// use serde_json::Value;
    ///
    /// let json: Value = isahc::get("https://httpbin.org/json")?.json()?;
    /// println!("author: {}", json["slideshow"]["author"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "json")]
    fn json<D>(&mut self) -> Result<D, serde_json::Error>
    where
        D: serde::de::DeserializeOwned,
        T: Read;
}

impl<T> ResponseExt<T> for Response<T> {
    fn effective_uri(&self) -> Option<&Uri> {
        self.extensions().get::<EffectiveUri>().map(|v| &v.0)
    }

    fn metrics(&self) -> Option<&Metrics> {
        self.extensions().get()
    }

    fn copy_to(&mut self, mut writer: impl Write) -> io::Result<u64>
    where
        T: Read,
    {
        io::copy(self.body_mut(), &mut writer)
    }

    #[cfg(feature = "text-decoding")]
    fn text(&mut self) -> io::Result<String>
    where
        T: Read,
    {
        crate::text::Decoder::for_response(&self).decode_reader(self.body_mut())
    }

    #[cfg(feature = "text-decoding")]
    fn text_async(&mut self) -> crate::text::TextFuture<'_, &mut T>
    where
        T: futures_io::AsyncRead + Unpin,
    {
        crate::text::Decoder::for_response(&self).decode_reader_async(self.body_mut())
    }

    #[cfg(feature = "json")]
    fn json<D>(&mut self) -> Result<D, serde_json::Error>
    where
        D: serde::de::DeserializeOwned,
        T: Read,
    {
        serde_json::from_reader(self.body_mut())
    }
}

pub(crate) struct EffectiveUri(pub(crate) Uri);
