//! The practical HTTP client that is fun to use.
//!
//! cHTTP is an HTTP client that provides a clean and easy-to-use interface
//! around the venerable [libcurl].
//!
//! ## Sending requests
//!
//! Sending requests is as easy as calling a single function. Let's make a
//! simple GET request to an example website:
//!
//! ```rust
//! use chttp;
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let mut response = chttp::get("https://example.org")?;
//! println!("{}", response.body_mut().text()?);
//! # Ok(())
//! # }
//! ```
//!
//! Requests are performed _synchronously_, up until the response headers are
//! received. The returned response struct includes the response body as an open
//! stream implementing `Read`.
//!
//! Sending a POST request is also easy, and takes an additional argument for
//! the request body:
//!
//! ```rust
//! use chttp;
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let response = chttp::post("https://example.org", "make me a salad")?;
//! # Ok(())
//! # }
//! ```
//!
//! cHTTP provides several other simple functions for common HTTP request types:
//!
//! ```rust
//! # use chttp;
//! #
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
//! use chttp::{self, http};
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let request = http::Request::post("https://example.org")
//!     .header("Content-Type", "application/json")
//!     .body(r#"{
//!         "speed": "fast",
//!         "cool_name": true
//!     }"#)?;
//! let response = chttp::send(request)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Request options
//!
//! How requests are sent can be customized using the
//! [`Options`](options/struct.Options.html) struct, which provides various
//! fields for setting timeouts, proxies, and other connection and protocol
//! configuration. These options can be included right along your request as an
//! extension object:
//!
//! ```rust
//! use chttp::{self, http, Options};
//! use std::time::Duration;
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let request = http::Request::get("https://example.org")
//!     .extension(Options::default()
//!         // Set a 5 second timeout.
//!         .with_timeout(Some(Duration::from_secs(5))))
//!     .body(())?;
//! let response = chttp::send(request)?;
//! # Ok(())
//! # }
//! ```
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
//! See the documentation for [`Client`](client/struct.Client.html) and
//! [`ClientBuilder`](client/struct.ClientBuilder.html) for more details on
//! creating custom clients.
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
//! ## Feature flags
//!
//! cHTTP is designed to be as "pay-as-you-need" as possible using Cargo feature
//! flags and optional dependencies. Unstable features are also initially
//! released behind feature flags until they are stabilized. You can add the
//! feature names below to your `Cargo.toml` file to enable them:
//!
//! ```toml
//! [dependencies.chttp]
//! version = "*"
//! features = ["async-api"]
//! ```
//!
//! Below is a list of all available feature flags and their meanings.
//!
//! ### `cookies`
//!
//! Enable persistent HTTP cookie support. Enabled by default.
//!
//! ### `http2`
//!
//! Enable HTTP/2 support in libcurl via libnghttp2. Enabled by default.
//!
//! ### `psl`
//!
//! Enable use of the Public Suffix List to filter out potentially malicious
//! cross-domain cookies. Enabled by default.
//!
//! ### `static-curl`
//!
//! Use a bundled libcurl version and statically link to it. Enabled by default.
//!
//! ### `async-api`
//!
//! Enable the async futures-based API. This allows you to take full advantage
//! of cHTTP's asynchronous core. Currently behind a feature flag until the
//! futures API stabilizes. This an unstable feature whose interface may change
//! between patch releases.
//!
//! ### `middleware-api`
//!
//! Enable the new middleware API. Unstable until the API is finalized. This an
//! unstable feature whose interface may change between patch releases.
//!
//! [libcurl]: https://curl.haxx.se/libcurl/
//! [log]: https://docs.rs/log

#![feature(async_await)]

use http::{Request, Response};
use lazy_static::lazy_static;
use std::future::Future;

pub mod client;
pub mod options;

#[cfg(feature = "cookies")]
pub mod cookies;

#[cfg(feature = "middleware-api")]
pub mod middleware;
#[cfg(not(feature = "middleware-api"))]
mod middleware;

mod agent;
mod body;
mod error;
mod handler;
mod parse;
mod wakers;

/// Re-export of the standard HTTP types.
pub extern crate http;

pub use crate::body::Body;
pub use crate::client::Client;
pub use crate::error::Error;
pub use crate::options::*;

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
/// The request may include [extensions](../http/struct.Extensions.html) to
/// customize how it is sent. You can include an
/// [`Options`](chttp::options::Options) struct as a request extension to
/// control various connection and protocol options.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn send<B: Into<Body>>(request: Request<B>) -> Result<Response<Body>, Error> {
    Client::shared().send(request)
}

/// Sends an HTTP request asynchronously.
///
/// The request may include [extensions](../http/struct.Extensions.html) to
/// customize how it is sent. You can include an
/// [`Options`](chttp::options::Options) struct as a request extension to
/// control various connection and protocol options.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn send_async<B: Into<Body>>(
    request: Request<B>,
) -> impl Future<Output = Result<Response<Body>, Error>> {
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
