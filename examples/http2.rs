use chttp::prelude::*;

fn main() -> Result<(), chttp::Error> {
    let response = Request::get("https://nghttp2.org")
        .preferred_http_version(chttp::http::Version::HTTP_2)
        .body(())
        .map_err(Into::into)
        .and_then(chttp::send)?;

    println!("{:?}", response.headers());

    Ok(())
}
