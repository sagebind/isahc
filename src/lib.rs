extern crate curl;
extern crate http;

pub mod client;
pub mod transfer;
pub mod transport;

pub use client::Client;

use std::io::Read;


pub type Request = http::Request<Entity>;
pub type Response = http::Response<Entity>;

/// Defines the body of an HTTP request or response.
pub enum Entity {
    /// An empty body.
    Empty,
    Bytes(Vec<u8>),
    String(String),
    Stream(Box<Read>),
}

pub fn get(uri: &str) -> Response {
    Client::new().get(uri)
}


pub enum Error {
    TransportBusy,
}
