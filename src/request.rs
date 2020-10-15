use std::convert::TryInto;

use crate::{
    client::ResponseFuture,
    config::{internal::ConfigurableBase, Configurable},
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

pub trait RequestBuilderExt {
    fn cookie(self, name: &str, value: &str) -> Self;
}

impl RequestBuilderExt for http::request::Builder {
    fn cookie(mut self, name: &str, value: &str) -> Self {
        if let Some(headers) = self.headers_mut() {
            if let Some(header_value) = headers.get_mut(http::header::COOKIE) {
                let mut new_header_value = header_value.as_bytes().to_vec();
                new_header_value.push(b';');
                new_header_value.push(b' ');
                new_header_value.extend_from_slice(name.as_bytes());
                new_header_value.push(b'=');
                new_header_value.extend_from_slice(value.as_bytes());

                if let Ok(new_header_value) = new_header_value.try_into() {
                    *header_value = new_header_value;
                }
            } else {
                let mut new_header_value = Vec::new();
                new_header_value.extend_from_slice(name.as_bytes());
                new_header_value.push(b'=');
                new_header_value.extend_from_slice(value.as_bytes());

                self = self.header(http::header::COOKIE, new_header_value);
            }
        }

        self
    }
}

impl Configurable for http::request::Builder {}

impl ConfigurableBase for http::request::Builder {
    fn configure(self, option: impl Send + Sync + 'static) -> Self {
        self.extension(option)
    }
}
