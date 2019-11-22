use isahc::config::VersionNegotiation;
use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
    let response = Request::get("https://nghttp2.org")
        .version_negotiation(VersionNegotiation::http2())
        .body(())
        .map_err(Into::into)
        .and_then(isahc::send)?;

    println!("{:?}", response.headers());

    Ok(())
}
