//! The practical HTTP client that is fun to use.
//!
//! ## Sending requests
//!
//! Sending requests is as easy as calling a single function. Let's make a
//! simple GET request to an example website:
//!
//! ```rust
//! use chttp::prelude::*;
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let mut response = chttp::get("https://example.org")?;
//! println!("{}", response.text()?);
//! # Ok(())
//! # }
//! ```
//!
//! By default, sending a request will wait for the response, up until the
//! response headers are received. The returned response struct includes the
//! response body as an open stream implementing [`Read`](std::io::Read).
//!
//! Sending a POST request is also easy, and takes an additional argument for
//! the request body:
//!
//! ```rust
//! # fn run() -> Result<(), chttp::Error> {
//! let response = chttp::post("https://example.org", "make me a salad")?;
//! # Ok(())
//! # }
//! ```
//!
//! cHTTP provides several other simple functions for common HTTP request types:
//!
//! ```rust
//! # fn run() -> Result<(), chttp::Error> {
//! chttp::put("https://example.org", "have a salad")?;
//! chttp::head("https://example.org")?;
//! chttp::delete("https://example.org")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom requests
//!
//! cHTTP is not limited to canned HTTP verbs; you can customize requests by
//! creating your own `Request` object and then `send`ing that.
//!
//! ```rust
//! use chttp::prelude::*;
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let response = Request::post("https://example.org")
//!     .header("Content-Type", "application/json")
//!     .body(r#"{
//!         "speed": "fast",
//!         "cool_name": true
//!     }"#)?
//!     .send()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Request configuration
//!
//! There are a number of options involved in request execution that can be
//! configured for a request, such as timeouts, proxies, and other connection
//! and protocol configuration . These can be customized by using extension
//! methods provided by the [`RequestBuilderExt`](prelude::RequestBuilderExt)
//! trait:
//!
//! ```rust
//! use chttp::prelude::*;
//! use std::time::Duration;
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let response = Request::get("https://example.org")
//!     .timeout(Duration::from_secs(5))
//!     .body(())?
//!     .send()?;
//! # Ok(())
//! # }
//! ```
//!
//! Configuration related to sending requests is stored inside the request
//! struct using [`http::Extensions`].
//!
//! ## Custom clients
//!
//! The free-standing functions for sending request delegate to a shared client
//! instance that is lazily instantiated with the default options. You can also
//! create custom client instances of your own, which allows you to set default
//! options for all requests and group related connections together. Each client
//! has its own connection pool and event loop, so separating certain requests
//! into separate clients can ensure that they are isolated from each other.
//!
//! See the documentation for [`Client`] and [`ClientBuilder`] for more details
//! on creating custom clients.
//!
//! ## Asynchronous API and execution
//!
//! Requests are always executed asynchronously under the hood. This allows a
//! single client to execute a large number of requests concurrently with
//! minimal overhead.
//!
//! If you are writing an asynchronous application, you can additionally benefit
//! from the async nature of the client by using the asynchronous methods
//! available to prevent blocking threads in your code. All request methods have
//! an asynchronous variant that ends with `_async` in the name. Here is our
//! first example rewritten to use async/await syntax (nightly only):
//!
//! ```rust
//! # #![cfg_attr(feature = "nightly", feature(async_await))]
//! # use chttp::prelude::*;
//! #
//! # #[cfg(feature = "nightly")]
//! # fn run() -> Result<(), chttp::Error> {
//! # futures::executor::block_on(async {
//! let mut response = chttp::get_async("https://example.org").await?;
//! println!("{}", response.text_async().await?);
//! # Ok(())
//! # })
//! # }
//! ```
//!
//! ## Logging
//!
//! cHTTP logs quite a bit of useful information at various levels using the
//! [log] crate.
//!
//! If you set the log level to `Trace` for the `chttp::wire` target, cHTTP will
//! also log all incoming and outgoing data while in flight. This may come in
//! handy if you are debugging code and need to see the exact data being sent to
//! the server and being received.
//!
//! [log]: https://docs.rs/log

use http::{Request, Response};
use lazy_static::lazy_static;
use std::future::Future;

#[cfg(feature = "cookies")]
pub mod cookies;

#[cfg(feature = "middleware-api")]
pub mod middleware;
#[cfg(not(feature = "middleware-api"))]
mod middleware;

mod agent;
mod body;
mod client;
pub mod config;
mod error;
mod handler;
mod io;
mod parse;
mod request;
mod response;
mod wakers;

/// Re-export of the standard HTTP types.
pub extern crate http;

pub use crate::body::Body;
pub use crate::client::{Client, ClientBuilder};
pub use crate::error::Error;

pub mod prelude {
    pub use crate::body::Body;
    pub use crate::client::{Client, ClientBuilder};
    pub use crate::request::{RequestBuilderExt, RequestExt};
    pub use crate::response::ResponseExt;
    pub use http::{Request, Response};
}

/// Sends an HTTP GET request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn get<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().get(uri)
}

/// Sends an HTTP GET request asynchronously.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn get_async<U>(uri: U) -> impl Future<Output = Result<Response<Body>, Error>>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().get_async(uri)
}

/// Sends an HTTP HEAD request.
pub fn head<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().head(uri)
}

/// Sends an HTTP HEAD request asynchronously.
pub fn head_async<U>(uri: U) -> impl Future<Output = Result<Response<Body>, Error>>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().head_async(uri)
}

/// Sends an HTTP POST request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn post<U>(uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().post(uri, body)
}

/// Sends an HTTP POST request asynchronously.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn post_async<U>(
    uri: U,
    body: impl Into<Body>,
) -> impl Future<Output = Result<Response<Body>, Error>>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().post_async(uri, body)
}

/// Sends an HTTP PUT request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn put<U>(uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().put(uri, body)
}

/// Sends an HTTP PUT request asynchronously.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn put_async<U>(
    uri: U,
    body: impl Into<Body>,
) -> impl Future<Output = Result<Response<Body>, Error>>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().put_async(uri, body)
}

/// Sends an HTTP DELETE request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn delete<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().delete(uri)
}

/// Sends an HTTP DELETE request asynchronously.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn delete_async<U>(uri: U) -> impl Future<Output = Result<Response<Body>, Error>>
where
    http::Uri: http::HttpTryFrom<U>,
{
    Client::shared().delete_async(uri)
}

/// Sends an HTTP request.
///
/// The request may include [extensions](http::Extensions) to
/// customize how it is sent. You can include an
/// [`Options`](crate::Options) struct as a request extension to
/// control various connection and protocol options.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn send<B: Into<Body>>(request: Request<B>) -> Result<Response<Body>, Error> {
    Client::shared().send(request)
}

/// Sends an HTTP request asynchronously.
///
/// The request may include [extensions](http::Extensions) to customize how it
/// is sent. You can include an [`Options`](crate::Options) struct as a request
/// extension to control various connection and protocol options.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn send_async<B: Into<Body>>(request: Request<B>) -> client::ResponseFuture<'static> {
    Client::shared().send_async(request)
}

/// Gets a human-readable string with the version number of cHTTP and its
/// dependencies.
///
/// This function can be helpful when troubleshooting issues in cHTTP or one of
/// its dependencies.
pub fn version() -> &'static str {
    static FEATURES_STRING: &'static str = include_str!(concat!(env!("OUT_DIR"), "/features.txt"));

    lazy_static! {
        static ref VERSION_STRING: String = format!(
            "chttp/{} (features:{}) {}",
            env!("CARGO_PKG_VERSION"),
            FEATURES_STRING,
            curl::Version::num(),
        );
    }

    &VERSION_STRING
}
