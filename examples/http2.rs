use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
    let response = Request::get("https://nghttp2.org")
        .preferred_http_version(isahc::http::Version::HTTP_2)
        .body(())
        .map_err(Into::into)
        .and_then(isahc::send)?;

    println!("{:?}", response.headers());

    Ok(())
}
