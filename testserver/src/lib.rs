//! HTTP server for testing.

#[macro_use]
mod macros;
mod mock;
mod pool;
mod request;
mod responder;
mod response;

pub mod socks4;

pub use macros::macro_api;
pub use mock::Mock;
pub use request::Request;
pub use responder::{RequestContext, Responder};
pub use response::Response;
