//! This example simply demonstrates HTTP/2 support by making a request that
//! enforces usage of HTTP/2.

use isahc::{
    config::VersionNegotiation,
    prelude::*,
};

fn main() -> Result<(), isahc::Error> {
    let response = Request::get("https://nghttp2.org")
        .version_negotiation(VersionNegotiation::http2())
        .body(())
        .map_err(Into::into)
        .and_then(isahc::send)?;

    println!("{:#?}", response.headers());

    Ok(())
}
