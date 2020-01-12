use crate::{
    client::ResponseFuture,
    config::{
        Configurable,
        internal::ConfigurableBase,
    },
    {Body, Error},
};
use http::{Request, Response};

/// Extension methods on an HTTP request.
pub trait RequestExt<T> {
    /// Send the HTTP request synchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send`](crate::send).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
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
        T: Into<Body>;
}

impl<T> RequestExt<T> for Request<T> {
    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>,
    {
        crate::send(self)
    }

    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<Body>,
    {
        crate::send_async(self)
    }
}

impl Configurable for http::request::Builder {}

impl ConfigurableBase for http::request::Builder {
    fn configure(self, option: impl Send + Sync + 'static) -> Self {
        self.extension(option)
    }
}
