use crate::{
    body::{AsyncBody, Body},
    client::ResponseFuture,
    config::{
        request::{RequestConfig, WithRequestConfig},
        Configurable,
    },
    error::Error,
};
use http::{Request, Response};

/// Extension methods on an HTTP request.
pub trait RequestExt<T> {
    /// Create a new request builder with the method, URI, and headers cloned
    /// from this request.
    ///
    /// Note that third-party extensions are not cloned.
    fn to_builder(&self) -> http::request::Builder;

    /// Send the HTTP request synchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send`](crate::send).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::{prelude::*, Request};
    ///
    /// let response = Request::post("https://httpbin.org/post")
    ///     .header("Content-Type", "application/json")
    ///     .body(r#"{
    ///         "speed": "fast",
    ///         "cool_name": true
    ///     }"#)?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>;

    /// Sends the HTTP request asynchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send_async`](crate::send_async).
    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<AsyncBody>;
}

impl<T> RequestExt<T> for Request<T> {
    fn to_builder(&self) -> http::request::Builder {
        let mut builder = Request::builder()
            .method(self.method().clone())
            .uri(self.uri().clone())
            .version(self.version());

        *builder.headers_mut().unwrap() = self.headers().clone();

        if let Some(config) = self.extensions().get::<RequestConfig>() {
            builder = builder.extension(config.clone());
        }

        #[cfg(feature = "cookies")]
        {
            if let Some(cookie_jar) = self.extensions().get::<crate::cookies::CookieJar>() {
                builder = builder.extension(cookie_jar.clone());
            }
        }

        builder
    }

    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>,
    {
        crate::send(self)
    }

    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<AsyncBody>,
    {
        crate::send_async(self)
    }
}

impl Configurable for http::request::Builder {
    #[cfg(feature = "cookies")]
    fn cookie_jar(self, cookie_jar: crate::cookies::CookieJar) -> Self {
        self.extension(cookie_jar)
    }
}

impl WithRequestConfig for http::request::Builder {
    #[inline]
    fn with_config(mut self, f: impl FnOnce(&mut RequestConfig)) -> Self {
        if let Some(extensions) = self.extensions_mut() {
            if let Some(config) = extensions.get_mut() {
                f(config);
            } else {
                extensions.insert(RequestConfig::default());
                f(extensions.get_mut().unwrap());
            }
        }

        self
    }
}
