use crate::{metrics::Metrics, redirect::EffectiveUri, trailer::Trailer};
use futures_lite::io::{copy as copy_async, AsyncRead, AsyncWrite};
use http::{Response, Uri};
use std::{
    fs::File,
    io::{self, Read, Write},
    net::SocketAddr,
    path::Path,
};

/// Provides extension methods for working with HTTP responses.
pub trait ResponseExt<T> {
    /// Get the trailer of the response containing headers that were received
    /// after the response body.
    ///
    /// See the documentation for [`Trailer`] for more details on how to handle
    /// trailing headers.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let mut response = isahc::get("https://my-site-with-trailers.com")?;
    ///
    /// println!("Status: {}", response.status());
    /// println!("Headers: {:#?}", response.headers());
    ///
    /// // Read and discard the response body until the end.
    /// response.consume()?;
    ///
    /// // Now the trailer will be available as well.
    /// println!("Trailing headers: {:#?}", response.trailer().try_get().unwrap());
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn trailer(&self) -> &Trailer;

    /// Get the effective URI of this response. This value differs from the
    /// original URI provided when making the request if at least one redirect
    /// was followed.
    ///
    /// This information is only available if populated by the HTTP client that
    /// produced the response.
    fn effective_uri(&self) -> Option<&Uri>;

    /// Get the local socket address of the last-used connection involved in
    /// this request, if known.
    ///
    /// Multiple connections may be involved in a request, such as with
    /// redirects.
    ///
    /// This method only makes sense with a normal Internet request. If some
    /// other kind of transport is used to perform the request, such as a Unix
    /// socket, then this method will return `None`.
    fn local_addr(&self) -> Option<SocketAddr>;

    /// Get the remote socket address of the last-used connection involved in
    /// this request, if known.
    ///
    /// Multiple connections may be involved in a request, such as with
    /// redirects.
    ///
    /// This method only makes sense with a normal Internet request. If some
    /// other kind of transport is used to perform the request, such as a Unix
    /// socket, then this method will return `None`.
    ///
    /// # Addresses and proxies
    ///
    /// The address returned by this method is the IP address and port that the
    /// client _connected to_ and not necessarily the real address of the origin
    /// server. Forward and reverse proxies between the caller and the server
    /// can cause the address to be returned to reflect the address of the
    /// nearest proxy rather than the server.
    fn remote_addr(&self) -> Option<SocketAddr>;

    /// Get the configured cookie jar used for persisting cookies from this
    /// response, if any.
    ///
    /// # Availability
    ///
    /// This method is only available when the [`cookies`](index.html#cookies)
    /// feature is enabled.
    #[cfg(feature = "cookies")]
    fn cookie_jar(&self) -> Option<&crate::cookies::CookieJar>;

    /// If request metrics are enabled for this particular transfer, return a
    /// metrics object containing a live view of currently available data.
    ///
    /// By default metrics are disabled and `None` will be returned. To enable
    /// metrics you can use
    /// [`Configurable::metrics`](crate::config::Configurable::metrics).
    fn metrics(&self) -> Option<&Metrics>;
}

impl<T> ResponseExt<T> for Response<T> {
    #[allow(clippy::redundant_closure)]
    fn trailer(&self) -> &Trailer {
        // Return a static empty trailer if the extension does not exist. This
        // offers a more convenient API so that users do not have to unwrap the
        // trailer from an extra Option.
        self.extensions().get().unwrap_or_else(|| Trailer::empty())
    }

    fn effective_uri(&self) -> Option<&Uri> {
        self.extensions().get::<EffectiveUri>().map(|v| &v.0)
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.extensions().get::<LocalAddr>().map(|v| v.0)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        self.extensions().get::<RemoteAddr>().map(|v| v.0)
    }

    #[cfg(feature = "cookies")]
    fn cookie_jar(&self) -> Option<&crate::cookies::CookieJar> {
        self.extensions().get()
    }

    fn metrics(&self) -> Option<&Metrics> {
        self.extensions().get()
    }
}

/// Provides extension methods for consuming HTTP response streams.
pub trait ReadResponseExt<R: Read> {
    /// Read any remaining bytes from the response body stream and discard them
    /// until the end of the stream is reached. It is usually a good idea to
    /// call this method before dropping a response if you know you haven't read
    /// the entire response body.
    ///
    /// # Background
    ///
    /// By default, if a response stream is dropped before it has been
    /// completely read from, then that HTTP connection will be terminated.
    /// Depending on which version of HTTP is being used, this may require
    /// closing the network connection to the server entirely. This can result
    /// in sub-optimal performance for making multiple requests, as it prevents
    /// Isahc from keeping the connection alive to be reused for subsequent
    /// requests.
    ///
    /// If you are downloading a file on behalf of a user and have been
    /// requested to cancel the operation, then this is probably what you want.
    /// But if you are making many small API calls to a known server, then you
    /// may want to call `consume()` before dropping the response, as reading a
    /// few megabytes off a socket is usually more efficient in the long run
    /// than taking a hit on connection reuse, and opening new connections can
    /// be expensive.
    ///
    /// Note that in HTTP/2 and newer, it is not necessary to close the network
    /// connection in order to interrupt the transfer of a particular response.
    /// If you know that you will be using only HTTP/2 or newer, then calling
    /// this method is probably unnecessary.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let mut response = isahc::get("https://example.org")?;
    ///
    /// println!("Status: {}", response.status());
    /// println!("Headers: {:#?}", response.headers());
    ///
    /// // Read and discard the response body until the end.
    /// response.consume()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn consume(&mut self) -> io::Result<()> {
        self.copy_to(io::sink())?;

        Ok(())
    }

    /// Copy the response body into a writer.
    ///
    /// Returns the number of bytes that were written.
    ///
    /// # Examples
    ///
    /// Copying the response into an in-memory buffer:
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let mut buf = vec![];
    /// isahc::get("https://example.org")?.copy_to(&mut buf)?;
    /// println!("Read {} bytes", buf.len());
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn copy_to<W: Write>(&mut self, writer: W) -> io::Result<u64>;

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
    fn copy_to_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<u64> {
        File::create(path).and_then(|f| self.copy_to(f))
    }

    /// Read the entire response body into memory.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let image_bytes = isahc::get("https://httpbin.org/image/jpeg")?.bytes()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn bytes(&mut self) -> io::Result<Vec<u8>>;

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
    fn text(&mut self) -> io::Result<String>;

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
    fn json<T>(&mut self) -> Result<T, serde_json::Error>
    where
        T: serde::de::DeserializeOwned;
}

impl<R: Read> ReadResponseExt<R> for Response<R> {
    fn copy_to<W: Write>(&mut self, mut writer: W) -> io::Result<u64> {
        io::copy(self.body_mut(), &mut writer)
    }

    fn bytes(&mut self) -> io::Result<Vec<u8>> {
        let mut buf = allocate_buffer(self);

        self.copy_to(&mut buf)?;

        Ok(buf)
    }

    #[cfg(feature = "text-decoding")]
    fn text(&mut self) -> io::Result<String> {
        crate::text::Decoder::for_response(self).decode_reader(self.body_mut())
    }

    #[cfg(feature = "json")]
    fn json<D>(&mut self) -> Result<D, serde_json::Error>
    where
        D: serde::de::DeserializeOwned,
    {
        serde_json::from_reader(self.body_mut())
    }
}

/// Provides extension methods for consuming asynchronous HTTP response streams.
pub trait AsyncReadResponseExt<R: AsyncRead + Unpin> {
    /// Read any remaining bytes from the response body stream and discard them
    /// until the end of the stream is reached. It is usually a good idea to
    /// call this method before dropping a response if you know you haven't read
    /// the entire response body.
    ///
    /// # Background
    ///
    /// By default, if a response stream is dropped before it has been
    /// completely read from, then that HTTP connection will be terminated.
    /// Depending on which version of HTTP is being used, this may require
    /// closing the network connection to the server entirely. This can result
    /// in sub-optimal performance for making multiple requests, as it prevents
    /// Isahc from keeping the connection alive to be reused for subsequent
    /// requests.
    ///
    /// If you are downloading a file on behalf of a user and have been
    /// requested to cancel the operation, then this is probably what you want.
    /// But if you are making many small API calls to a known server, then you
    /// may want to call `consume()` before dropping the response, as reading a
    /// few megabytes off a socket is usually more efficient in the long run
    /// than taking a hit on connection reuse, and opening new connections can
    /// be expensive.
    ///
    /// Note that in HTTP/2 and newer, it is not necessary to close the network
    /// connection in order to interrupt the transfer of a particular response.
    /// If you know that you will be using only HTTP/2 or newer, then calling
    /// this method is probably unnecessary.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// # async fn run() -> Result<(), isahc::Error> {
    /// let mut response = isahc::get_async("https://example.org").await?;
    ///
    /// println!("Status: {}", response.status());
    /// println!("Headers: {:#?}", response.headers());
    ///
    /// // Read and discard the response body until the end.
    /// response.consume().await?;
    /// # Ok(()) }
    /// ```
    fn consume(&mut self) -> ConsumeFuture<'_, R>;

    /// Copy the response body into a writer asynchronously.
    ///
    /// Returns the number of bytes that were written.
    ///
    /// # Examples
    ///
    /// Copying the response into an in-memory buffer:
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// # async fn run() -> Result<(), isahc::Error> {
    /// let mut buf = vec![];
    /// isahc::get_async("https://example.org").await?
    ///     .copy_to(&mut buf).await?;
    /// println!("Read {} bytes", buf.len());
    /// # Ok(()) }
    /// ```
    fn copy_to<'a, W>(&'a mut self, writer: W) -> CopyFuture<'a, R, W>
    where
        W: AsyncWrite + Unpin + 'a;

    /// Read the entire response body into memory.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// # async fn run() -> Result<(), isahc::Error> {
    /// let image_bytes = isahc::get_async("https://httpbin.org/image/jpeg")
    ///     .await?
    ///     .bytes()
    ///     .await?;
    /// # Ok(()) }
    /// ```
    fn bytes(&mut self) -> BytesFuture<'_, &mut R>;

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
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// # async fn run() -> Result<(), isahc::Error> {
    /// let text = isahc::get_async("https://example.org").await?
    ///     .text().await?;
    /// println!("{}", text);
    /// # Ok(()) }
    /// ```
    #[cfg(feature = "text-decoding")]
    fn text(&mut self) -> crate::text::TextFuture<'_, &mut R>;

    /// Deserialize the response body as JSON into a given type.
    ///
    /// # Caveats
    ///
    /// Unlike its [synchronous equivalent](ReadResponseExt::json), this method
    /// reads the entire response body into memory before attempting
    /// deserialization. This is due to a Serde limitation since incremental
    /// partial deserializing is not supported.
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
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let json: Value = isahc::get_async("https://httpbin.org/json").await?
    ///     .json().await?;
    /// println!("author: {}", json["slideshow"]["author"]);
    /// # Ok(()) }
    /// ```
    #[cfg(feature = "json")]
    fn json<T>(&mut self) -> JsonFuture<'_, R, T>
    where
        T: serde::de::DeserializeOwned;
}

impl<R: AsyncRead + Unpin> AsyncReadResponseExt<R> for Response<R> {
    fn consume(&mut self) -> ConsumeFuture<'_, R> {
        ConsumeFuture::new(async move {
            copy_async(self.body_mut(), futures_lite::io::sink()).await?;

            Ok(())
        })
    }

    fn copy_to<'a, W>(&'a mut self, writer: W) -> CopyFuture<'a, R, W>
    where
        W: AsyncWrite + Unpin + 'a,
    {
        CopyFuture::new(async move { copy_async(self.body_mut(), writer).await })
    }

    fn bytes(&mut self) -> BytesFuture<'_, &mut R> {
        BytesFuture::new(async move {
            let mut buf = allocate_buffer(self);

            copy_async(self.body_mut(), &mut buf).await?;

            Ok(buf)
        })
    }

    #[cfg(feature = "text-decoding")]
    fn text(&mut self) -> crate::text::TextFuture<'_, &mut R> {
        crate::text::Decoder::for_response(self).decode_reader_async(self.body_mut())
    }

    #[cfg(feature = "json")]
    fn json<T>(&mut self) -> JsonFuture<'_, R, T>
    where
        T: serde::de::DeserializeOwned,
    {
        JsonFuture::new(async move {
            let mut buf = allocate_buffer(self);

            // Serde does not support incremental parsing, so we have to resort
            // to reading the entire response into memory first and then
            // deserializing.
            if let Err(e) = copy_async(self.body_mut(), &mut buf).await {
                struct ErrorReader(Option<io::Error>);

                impl Read for ErrorReader {
                    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                        Err(self.0.take().unwrap())
                    }
                }

                // Serde offers no public way to directly create an error from
                // an I/O error, but we can do so in a roundabout way by parsing
                // a reader that always returns the desired error.
                serde_json::from_reader(ErrorReader(Some(e)))
            } else {
                serde_json::from_slice(&buf)
            }
        })
    }
}

fn allocate_buffer<T>(response: &Response<T>) -> Vec<u8> {
    if let Some(length) = get_content_length(response) {
        Vec::with_capacity(length as usize)
    } else {
        Vec::new()
    }
}

fn get_content_length<T>(response: &Response<T>) -> Option<u64> {
    response.headers()
        .get(http::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

decl_future! {
    /// A future which reads any remaining bytes from the response body stream
    /// and discard them.
    pub type ConsumeFuture<R> = impl Future<Output = io::Result<()>> + SendIf<R>;

    /// A future which copies all the response body bytes into a sink.
    pub type CopyFuture<R, W> = impl Future<Output = io::Result<u64>> + SendIf<R, W>;

    /// A future which reads the entire response body into memory.
    pub type BytesFuture<R> = impl Future<Output = io::Result<Vec<u8>>> + SendIf<R>;

    /// A future which deserializes the response body as JSON.
    #[cfg(feature = "json")]
    pub type JsonFuture<R, T> = impl Future<Output = Result<T, serde_json::Error>> + SendIf<R, T>;
}

pub(crate) struct LocalAddr(pub(crate) SocketAddr);

pub(crate) struct RemoteAddr(pub(crate) SocketAddr);

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(CopyFuture<'static, Vec<u8>, Vec<u8>>: Send);

    // *mut T is !Send
    static_assertions::assert_not_impl_any!(CopyFuture<'static, *mut Vec<u8>, Vec<u8>>: Send);
    static_assertions::assert_not_impl_any!(CopyFuture<'static, Vec<u8>, *mut Vec<u8>>: Send);
    static_assertions::assert_not_impl_any!(CopyFuture<'static, *mut Vec<u8>, *mut Vec<u8>>: Send);
}
