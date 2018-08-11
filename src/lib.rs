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
//! use chttp::{self, http, Body};
//!
//! # fn run() -> Result<(), chttp::Error> {
//! let request = http::Request::post("https://example.org")
//!     .header("Content-Type", "application/json")
//!     .body(Body::from(r#"{
//!         "speed": "fast",
//!         "cool_name": true
//!     }"#))?;
//! let response = chttp::send(request)?;
//! # Ok(())
//! # }
//! ```
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

extern crate curl;
extern crate futures;
pub extern crate http;
#[cfg(feature = "json")]
extern crate json;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[cfg(unix)]
extern crate nix;
extern crate ringtail;
extern crate slab;

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

/// Sends a GET request.
pub fn get(uri: &str) -> Result<Response, Error> {
    DEFAULT_CLIENT.get(uri)
}

/// Sends a HEAD request.
pub fn head(uri: &str) -> Result<Response, Error> {
    DEFAULT_CLIENT.head(uri)
}

/// Sends a POST request.
pub fn post<B: Into<Body>>(uri: &str, body: B) -> Result<Response, Error> {
    DEFAULT_CLIENT.post(uri, body)
}

/// Sends a PUT request.
pub fn put<B: Into<Body>>(uri: &str, body: B) -> Result<Response, Error> {
    DEFAULT_CLIENT.put(uri, body)
}

/// Sends a DELETE request.
pub fn delete(uri: &str) -> Result<Response, Error> {
    DEFAULT_CLIENT.delete(uri)
}

/// Sends a request.
pub fn send(request: Request) -> Result<Response, Error> {
    DEFAULT_CLIENT.send(request)
}
