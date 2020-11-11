//! The practical HTTP client that is fun to use.
//!
//! Here are some of Isahc's key features:
//!
//! - Full support for HTTP/1.1 and HTTP/2.
//! - Configurable request timeouts.
//! - Fully asynchronous core, with asynchronous and incremental reading and
//!   writing of request and response bodies.
//! - Offers an ergonomic synchronous API as well as an asynchronous API with
//!   support for async/await.
//! - Optional automatic redirect following.
//! - Sessions and cookie persistence.
//!
//! # Getting started
//!
//! Sending requests is as easy as calling a single function. Let's make a
//! simple GET request to an example website:
//!
//! ```no_run
//! use isahc::prelude::*;
//!
//! let mut response = isahc::get("https://example.org")?;
//! println!("{}", response.text()?);
//! # Ok::<(), isahc::Error>(())
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
//! let response = isahc::post("https://httpbin.org/post", "make me a salad")?;
//! # Ok::<(), isahc::Error>(())
//! ```
//!
//! Isahc provides several other simple functions for common HTTP request types:
//!
//! ```no_run
//! isahc::put("https://httpbin.org/put", "have a salad")?;
//! isahc::head("https://httpbin.org/get")?;
//! isahc::delete("https://httpbin.org/delete")?;
//! # Ok::<(), isahc::Error>(())
//! ```
//!
//! If you want to customize the request by adding headers, setting timeouts,
//! etc, then you can create a [`Request`][prelude::Request] using a
//! builder-style fluent interface, then finishing it off with a
//! [`send`][RequestExt::send]:
//!
//! ```no_run
//! use isahc::prelude::*;
//! use std::time::Duration;
//!
//! let response = Request::post("https://httpbin.org/post")
//!     .header("Content-Type", "application/json")
//!     .timeout(Duration::from_secs(5))
//!     .body(r#"{
//!         "speed": "fast",
//!         "cool_name": true
//!     }"#)?
//!     .send()?;
//! # Ok::<(), isahc::Error>(())
//! ```
//!
//! For even more examples used in complete programs, please check out the
//! [examples](https://github.com/sagebind/isahc/tree/master/examples) directory
//! in the project repo.
//!
//! # Feature tour
//!
//! Below is a brief overview of some notable features of Isahc. Check out the
//! rest of the documentation for even more guides and examples.
//!
//! ## Easy request functions
//!
//! You can start sending requests without any configuration by using the global
//! functions in this module, including [`get`], [`post`], and [`send`]. These
//! use a shared HTTP client instance with sane defaults, so it is easy to get
//! up and running. They should work perfectly fine for many use-cases, so don't
//! worry about graduating to more complex APIs if you don't need them.
//!
//! ## Request and response traits
//!
//! Isahc includes a number of traits in the [`prelude`] module that extend the
//! [`Request`] and [`Response`] types with a plethora of extra methods that
//! make common tasks convenient and allow you to configure more advanced
//! connection and protocol details.
//!
//! Some key traits to read about include
//! [`Configurable`](config::Configurable), [`RequestExt`], and [`ResponseExt`].
//!
//! ## Custom clients
//!
//! The free-standing functions for sending requests use a shared [`HttpClient`]
//! instance, but you can also create your own client instances, which allows
//! you to customize the default behavior for requests that use it.
//!
//! See the documentation for [`HttpClient`] and [`HttpClientBuilder`] for more
//! information on creating custom clients.
//!
//! ## Asynchronous requests
//!
//! Requests are always executed asynchronously under the hood. This allows a
//! single client to execute a large number of requests concurrently with
//! minimal overhead. Even synchronous applications can benefit!
//!
//! If you are writing an asynchronous application, you can reap additional
//! benefits from the async nature of the client by using the asynchronous
//! methods available to prevent blocking threads in your code. All request
//! methods have an asynchronous variant that ends with `_async` in the name.
//! Here is our first example rewritten to use async/await syntax:
//!
//! ```no_run
//! # async fn run() -> Result<(), isahc::Error> {
//! use isahc::prelude::*;
//!
//! let mut response = isahc::get_async("https://httpbin.org/get").await?;
//! println!("{}", response.text_async().await?);
//! # Ok(()) }
//! ```
//!
//! # Feature flags
//!
//! Isahc is designed to be as "pay-as-you-need" as possible using Cargo feature
//! flags and optional dependencies. Unstable features are also initially
//! released behind feature flags until they are stabilized. You can add the
//! feature names below to your `Cargo.toml` file to enable them:
//!
//! ```toml
//! [dependencies.isahc]
//! version = "0.8"
//! features = ["psl"]
//! ```
//!
//! Below is a list of all available feature flags and their meanings.
//!
//! ## `cookies`
//!
//! Enable persistent HTTP cookie support. Disabled by default.
//!
//! ## `http2`
//!
//! Enable compile-time support for HTTP/2 in libcurl via libnghttp2. This does
//! not actually affect whether HTTP/2 is used for a given request, but simply
//! makes it available. To configure which HTTP versions to use in a request,
//! see [`VersionNegotiation`](config::VersionNegotiation).
//!
//! Enabled by default.
//!
//! ## `json`
//!
//! Additional serialization and deserialization of JSON bodies via
//! [serde](https://serde.rs). Disabled by default.
//!
//! ## `psl`
//!
//! Enable use of the Public Suffix List to filter out potentially malicious
//! cross-domain cookies. Implies `cookies`, disabled by default.
//!
//! ## `spnego`
//!
//! Enable support for [SPNEGO-based HTTP
//! authentication](https://tools.ietf.org/html/rfc4559) (`negotiate` auth
//! scheme). This makes the `negotiate` scheme available in the API and, if
//! `static-curl` is enabled, compiles libcurl with GSS-API APIs. The [MIT
//! Kerberos](https://web.mit.edu/kerberos/) headers must be pre-installed at
//! compile time.
//!
//! ## `static-curl`
//!
//! Use a bundled libcurl version and statically link to it. Enabled by default.
//!
//! ## `text-decoding`
//!
//! Enable support for decoding text-based responses in various charsets into
//! strings. Enabled by default.
//!
//! ## Unstable APIs
//!
//! There are also some features that enable new incubating APIs that do not
//! have stability guarantees:
//!
//! ### `unstable-interceptors`
//!
//! Enable the new interceptors API (replaces the old unstable middleware API).
//! Unstable until the API is finalized. This an unstable feature whose
//! interface may change between patch releases.
//!
//! # Logging and tracing
//!
//! Isahc logs quite a bit of useful information at various levels compatible
//! with the [log](https://docs.rs/log) crate. For even more in-depth
//! diagnostics, you can use a [tracing](https://docs.rs/tracing) subscriber to
//! track log events grouped by individual requests. This can be especially
//! useful if you are sending multiple requests concurrently.
//!
//! If you set the log level to `Trace` for the `isahc::wire` target, Isahc will
//! also log all incoming and outgoing data while in flight. This may come in
//! handy if you are debugging code and need to see the exact data being sent to
//! the server and being received.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/sagebind/isahc/master/media/isahc.svg.png",
    html_favicon_url = "https://raw.githubusercontent.com/sagebind/isahc/master/media/icon.png"
)]
#![deny(unsafe_code)]
#![warn(
    future_incompatible,
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    unused,
    clippy::all
)]
// This lint produces a lot of false positives. See
// https://github.com/rust-lang/rust-clippy/issues/3900.
#![allow(clippy::cognitive_complexity)]

use http::{Request, Response};
use once_cell::sync::Lazy;
use std::convert::TryFrom;

#[cfg(feature = "cookies")]
pub mod cookies;

mod agent;
mod body;
mod client;
mod default_headers;
mod error;
mod handler;
mod headers;
mod metrics;
mod redirect;
mod request;
mod response;
mod task;
mod text;

pub mod auth;
pub mod config;

#[cfg(feature = "unstable-interceptors")]
pub mod interceptor;
#[cfg(not(feature = "unstable-interceptors"))]
#[allow(unreachable_pub, unused)]
pub(crate) mod interceptor;

pub use crate::{
    body::Body,
    client::{HttpClient, HttpClientBuilder, ResponseFuture},
    error::Error,
    metrics::Metrics,
    request::RequestExt,
    response::ResponseExt,
};

/// Re-export of the standard HTTP types.
pub use http;

/// A "prelude" for importing common Isahc types.
///
/// # Example
///
/// ```
/// use isahc::prelude::*;
/// ```
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{config::Configurable, Body, HttpClient, RequestExt, ResponseExt};

    #[doc(no_inline)]
    pub use http::{Request, Response};
}

/// Send a GET request to the given URI.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::get`] for details.
pub fn get<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().get(uri)
}

/// Send a GET request to the given URI asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::get_async`] for details.
pub fn get_async<U>(uri: U) -> ResponseFuture<'static>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().get_async(uri)
}

/// Send a HEAD request to the given URI.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::head`] for details.
pub fn head<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().head(uri)
}

/// Send a HEAD request to the given URI asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::head_async`] for details.
pub fn head_async<U>(uri: U) -> ResponseFuture<'static>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().head_async(uri)
}

/// Send a POST request to the given URI with a given request body.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::post`] for details.
pub fn post<U>(uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
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
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().post_async(uri, body)
}

/// Send a PUT request to the given URI with a given request body.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::put`] for details.
pub fn put<U>(uri: U, body: impl Into<Body>) -> Result<Response<Body>, Error>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
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
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().put_async(uri, body)
}

/// Send a DELETE request to the given URI.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::delete`] for details.
pub fn delete<U>(uri: U) -> Result<Response<Body>, Error>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    HttpClient::shared().delete(uri)
}

/// Send a DELETE request to the given URI asynchronously.
///
/// The request is executed using a shared [`HttpClient`] instance. See
/// [`HttpClient::delete_async`] for details.
pub fn delete_async<U>(uri: U) -> ResponseFuture<'static>
where
    http::Uri: TryFrom<U>,
    <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
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

/// Gets a human-readable string with the version number of Isahc and its
/// dependencies.
///
/// This function can be helpful when troubleshooting issues in Isahc or one of
/// its dependencies.
pub fn version() -> &'static str {
    static FEATURES_STRING: &str = include_str!(concat!(env!("OUT_DIR"), "/features.txt"));
    static VERSION_STRING: Lazy<String> = Lazy::new(|| format!(
        "isahc/{} (features:{}) {}",
        env!("CARGO_PKG_VERSION"),
        FEATURES_STRING,
        curl::Version::num(),
    ));

    &VERSION_STRING
}
