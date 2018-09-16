#![deny(missing_docs)]

//! The practical HTTP client that is fun to use.
//!
//! cHTTP is an HTTP client that provides a clean and easy-to-use interface around the venerable [libcurl].
//!
//! ## Sending requests
//!
//! Sending requests is as easy as calling a single function. Let's make a simple GET request to an example website:
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
//! Requests are performed _synchronously_, up until the response headers are received. The returned response struct
//! includes the response body as an open stream implementing `Read`.
//!
//! Sending a POST request is also easy, and takes an additional argument for the request body:
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
//! cHTTP is not limited to canned HTTP verbs; you can customize requests by creating your own `Request` object and then
//! `send`ing that.
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
//! How requests are sent can be customized using the [`Options`](options/struct.Options.html) struct, which provides various
//! fields for setting timeouts, proxies, and other connection and protocol configuration. These options can be included
//! right along your request as an extension object:
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
//! The free-standing functions for sending request delegate to a shared client instance that is lazily instantiated
//! with the default options. You can also create custom client instances of your own, which allows you to set default
//! options for all requests and group related connections together. Each client has its own connection pool and event
//! loop, so separating certain requests into separate clients can ensure that they are isolated from each other.
//!
//! See the documentation for [`Client`](client/struct.Client.html) and
//! [`ClientBuilder`](client/struct.ClientBuilder.html) for more details on creating custom clients.
//!
//! ## Logging
//!
//! cHTTP logs quite a bit of useful information at various levels using the [log] crate.
//!
//! If you set the log level to `Trace` for the `chttp::wire` target, cHTTP will also log all incoming and outgoing data
//! while in flight. This may come in handy if you are debugging code and need to see the exact data being sent to the
//! server and being received.
//!
//! [libcurl]: https://curl.haxx.se/libcurl/
//! [log]: https://docs.rs/log

extern crate bytes;
extern crate crossbeam_channel;
extern crate curl;
extern crate futures;
pub extern crate http;
#[cfg(feature = "json")]
extern crate json;
#[macro_use]
extern crate lazy_static;
extern crate lazycell;
#[macro_use]
extern crate log;
#[cfg(unix)]
extern crate nix;
extern crate regex;
extern crate slab;
#[macro_use]
extern crate withers_derive;

pub mod body;
pub mod client;
pub mod error;
pub mod options;

mod internal;

pub use body::Body;
pub use client::Client;
pub use error::Error;
pub use options::*;

/// An HTTP request.
pub type Request = http::Request<Body>;

/// An HTTP response.
pub type Response = http::Response<Body>;

lazy_static! {
    static ref DEFAULT_CLIENT: Client = Client::new().unwrap();
}

/// Sends an HTTP GET request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn get<U>(uri: U) -> Result<Response, Error> where http::Uri: http::HttpTryFrom<U> {
    DEFAULT_CLIENT.get(uri)
}

/// Sends an HTTP HEAD request.
pub fn head<U>(uri: U) -> Result<Response, Error> where http::Uri: http::HttpTryFrom<U> {
    DEFAULT_CLIENT.head(uri)
}

/// Sends an HTTP POST request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn post<U>(uri: U, body: impl Into<Body>) -> Result<Response, Error> where http::Uri: http::HttpTryFrom<U> {
    DEFAULT_CLIENT.post(uri, body)
}

/// Sends an HTTP PUT request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn put<U>(uri: U, body: impl Into<Body>) -> Result<Response, Error> where http::Uri: http::HttpTryFrom<U> {
    DEFAULT_CLIENT.put(uri, body)
}

/// Sends an HTTP DELETE request.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn delete<U>(uri: U) -> Result<Response, Error> where http::Uri: http::HttpTryFrom<U> {
    DEFAULT_CLIENT.delete(uri)
}

/// Sends an HTTP request.
///
/// The request may include [extensions](../http/struct.Extensions.html) to customize how it is sent. You can include an
/// [`Options`](chttp::options::Options) struct as a request extension to control various connection and protocol
/// options.
///
/// The response body is provided as a stream that may only be consumed once.
pub fn send<B: Into<Body>>(request: http::Request<B>) -> Result<Response, Error> {
    DEFAULT_CLIENT.send(request.map(|body| body.into()))
}
