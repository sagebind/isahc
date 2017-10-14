extern crate curl;
extern crate http;
#[cfg(feature = "json")]
extern crate json;

pub mod body;
pub mod client;
pub mod error;
pub mod transport;

pub use body::Body;
pub use client::Client;
pub use error::Error;


pub type Request = http::Request<Body>;
pub type Response = http::Response<Body>;


/// Sends a GET request.
pub fn get(uri: &str) -> Result<Response, Error> {
    Client::new().get(uri)
}

/// Sends a POST request.
pub fn post<B: Into<Body>>(uri: &str, body: B) -> Result<Response, Error> {
    Client::new().post(uri, body)
}

/// Sends a PUT request.
pub fn put<B: Into<Body>>(uri: &str, body: B) -> Result<Response, Error> {
    Client::new().put(uri, body)
}

/// Sends a DELETE request.
pub fn delete(uri: &str) -> Result<Response, Error> {
    Client::new().delete(uri)
}
