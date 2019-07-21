//! The practical HTTP client that is fun to use.
//!
//! # Sending requests
//!
//! Sending requests is as easy as calling a single function. Let's make a
//! simple GET request to an example website:
//!
//! ```no_run
//! use chttp::prelude::*;
//!
//! let mut response = chttp::get("https://example.org")?;
//! println!("{}", response.text()?);
//! # Ok::<(), chttp::Error>(())
//! ```
//!
//! By default, sending a request will wait for the response, up until the
//! response headers are received. The returned response struct includes the
//! response body as an open stream implementing [`Read`](std::io::Read).
//!
//! Sending a POST request is also easy, and takes an additional argument for
//! the request body:
//!
//! ```no_run
//! let response = chttp::post("https://httpbin.org/post", "make me a salad")?;
//! # Ok::<(), chttp::Error>(())
//! ```
//!
//! cHTTP provides several other simple functions for common HTTP request types:
//!
//! ```no_run
//! chttp::put("https://httpbin.org/put", "have a salad")?;
//! chttp::head("https://httpbin.org/get")?;
//! chttp::delete("https://httpbin.org/delete")?;
//! # Ok::<(), chttp::Error>(())
//! ```
//!
//! # Custom requests
//!
//! cHTTP is not limited to canned HTTP verbs; you can customize requests by
//! creating your own `Request` object and then `send`ing that.
//!
//! ```no_run
//! use chttp::prelude::*;
//!
//! let response = Request::post("https://httpbin.org/post")
//!     .header("Content-Type", "application/json")
//!     .body(r#"{
//!         "speed": "fast",
//!         "cool_name": true
//!     }"#)?
//!     .send()?;
//! # Ok::<(), chttp::Error>(())
//! ```
//!
//! # Request configuration
//!
//! There are a number of options involved in request execution that can be
//! configured for a request, such as timeouts, proxies, and other connection
//! and protocol configuration. These can be customized by using extension
//! methods provided by the [`RequestBuilderExt`](prelude::RequestBuilderExt)
//! trait:
//!
//! ```no_run
//! use chttp::prelude::*;
//! use std::time::Duration;
//!
//! let response = Request::get("https://httpbin.org/get")
//!     .timeout(Duration::from_secs(5))
//!     .body(())?
//!     .send()?;
//! # Ok::<(), chttp::Error>(())
//! ```
//!
//! Configuration related to sending requests is stored inside the request
//! struct using [`http::Extensions`].
//!
//! # Custom clients
//!
//! The free-standing functions for sending request delegate to a shared client
//! instance that is lazily instantiated with the default options. You can also
//! create custom client instances of your own, which allows you to set default
//! options for all requests and group related connections together. Each client
//! has its own connection pool and event loop, so separating certain requests
//! into separate clients can ensure that they are isolated from each other.
//!
//! See the documentation for [`HttpClient`] and [`HttpClientBuilder`] for more
//! details on creating custom clients.
//!
//! # Asynchronous API and execution
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
//! ```no_run
//! # #![cfg_attr(feature = "nightly", feature(async_await))]
//! # use chttp::prelude::*;
//! #
//! # #[cfg(feature = "nightly")]
//! # fn run() -> Result<(), chttp::Error> {
//! # futures::executor::block_on(async {
//! let mut response = chttp::get_async("https://httpbin.org/get").await?;
//! println!("{}", response.text_async().await?);
//! # Ok(())
//! # })
//! # }
//! ```
//!
//! # Logging
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

#![deny(unsafe_code)]
#![warn(
    future_incompatible,
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    unused,
    clippy::all,
)]

use http::{Request, Response};
use lazy_static::lazy_static;

#[cfg(feature = "cookies")]
pub mod cookies;

#[cfg(feature = "middleware-api")]
pub mod middleware;
#[cfg(not(feature = "middleware-api"))]
#[allow(unreachable_pub, unused)]
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

pub use crate::{
    body::Body,
    client::{HttpClient, HttpClientBuilder, ResponseFuture},
    error::Error,
};

/// Re-export of the standard HTTP types.
pub use http;

/// A "prelude" for importing common cHTTP types.
pub mod prelude {
    pub use crate::{
        Body,
        HttpClient,
        request::{RequestBuilderExt, RequestExt},
        response::ResponseExt,
    };

    pub use http::{Request, Response};
}

/// Send a GET request to the given URI.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::get`] for details.
pub fn get<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().get(uri)
}

/// Send a GET request to the given URI asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::get_async`] for details.
pub fn get_async<U>(uri: U) -> ResponseFuture<'static>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().get_async(uri)
}

/// Send a HEAD request to the given URI.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::head`] for details.
pub fn head<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().head(uri)
}

/// Send a HEAD request to the given URI asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::head_async`] for details.
pub fn head_async<U>(uri: U) -> ResponseFuture<'static>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().head_async(uri)
}

/// Send a POST request to the given URI with a given request body.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::post`] for details.
pub fn post<U>(uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().post(uri, body)
}

/// Send a POST request to the given URI asynchronously with a given request
/// body.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::post_async`] for details.
pub fn post_async<U>(uri: U, body: impl Into<Body>) -> ResponseFuture<'static>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().post_async(uri, body)
}

/// Send a PUT request to the given URI with a given request body.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::put`] for details.
pub fn put<U>(uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().put(uri, body)
}

/// Send a PUT request to the given URI asynchronously with a given request
/// body.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::put_async`] for details.
pub fn put_async<U>(uri: U, body: impl Into<Body>) -> ResponseFuture<'static>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().put_async(uri, body)
}

/// Send a DELETE request to the given URI.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::delete`] for details.
pub fn delete<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().delete(uri)
}

/// Send a DELETE request to the given URI asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::delete_async`] for details.
pub fn delete_async<U>(uri: U) -> ResponseFuture<'static>
where
    http::Uri: http::HttpTryFrom<U>,
{
    HttpClient::shared().delete_async(uri)
}

/// Send an HTTP request and return the HTTP response.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::send`] for details.
pub fn send<B: Into<Body>>(request: Request<B>) -> Result<Response<Body>, Error> {
    HttpClient::shared().send(request)
}

/// Send an HTTP request and return the HTTP response asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::send_async`] for details.
pub fn send_async<B: Into<Body>>(request: Request<B>) -> ResponseFuture<'static> {
    HttpClient::shared().send_async(request)
}

/// Gets a human-readable string with the version number of cHTTP and its
/// dependencies.
///
/// This function can be helpful when troubleshooting issues in cHTTP or one of
/// its dependencies.
pub fn version() -> &'static str {
    static FEATURES_STRING: &str = include_str!(concat!(env!("OUT_DIR"), "/features.txt"));

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
