use crate::{
    text::Decoder,
    Metrics,
};
use futures_io::AsyncRead;
use futures_util::{
    future::{FutureExt, LocalBoxFuture},
    io::{AsyncReadExt},
};
use http::{Response, Uri};
use std::{
    fs::File,
    future::Future,
    io::{self, Read, Write},
    path::Path,
    pin::Pin,
    task::{Context, Poll},
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

    /// Get the response body as a string.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
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
    fn text(&mut self) -> io::Result<String>
    where
        T: Read;

    /// Get the response body as a string asynchronously.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
    fn text_async(&mut self) -> Text<'_>
    where
        T: AsyncRead + Unpin;

    /// Deserialize the response body as JSON into a given type.
    ///
    /// This method requires the `json` feature to be enabled.
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

macro_rules! read_text_impl {
    ($response:expr, $buf:ident, $read:expr) => {
        {
            let mut decoder = Decoder::new(guess_encoding($response).unwrap_or(encoding_rs::UTF_8));
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

                unread = decoder.push(&buf[..unread+len]).len();
            }

            Ok(decoder.finish(&buf[..unread]))
        }
    };
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

    fn text(&mut self) -> io::Result<String>
    where
        T: Read,
    {
        read_text_impl!(self, buf, self.body_mut().read(buf))
    }

    fn text_async(&mut self) -> Text<'_>
    where
        T: AsyncRead + Unpin,
    {
        Text(Box::pin(async move {
            read_text_impl!(self, buf, self.body_mut().read(buf).await)
        }))
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

/// A future returning a response body decoded as text.
#[allow(missing_debug_implementations)]
pub struct Text<'a>(LocalBoxFuture<'a, io::Result<String>>);

impl<'a> Future for Text<'a> {
    type Output = io::Result<String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.as_mut().0.poll_unpin(cx)
    }
}

fn guess_encoding<T>(response: &Response<T>) -> Option<&'static encoding_rs::Encoding> {
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)?
        .to_str()
        .ok()?
        .parse::<mime::Mime>()
        .ok()?;

    encoding_rs::Encoding::for_label(content_type.get_param("charset")?.as_str().as_bytes())
}

pub(crate) struct EffectiveUri(pub(crate) Uri);
