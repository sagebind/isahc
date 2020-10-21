//! A simple example program that sends a GET request and then prints out all
//! the cookies in the cookie jar.

use isahc::cookies::CookieJar;
use isahc::prelude::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Create a new cookie jar.
    let cookie_jar = CookieJar::new();

    // Send a request to a server that sets a cookie.
    let uri = "http://httpbin.org/cookies/set?foo=bar&baz=123".parse()?;
    let _response = Request::get(&uri)
        // Set the cookie jar to use for this request.
        .cookie_jar(cookie_jar.clone())
        .body(())?
        .send()?;

    // Print all cookies relevant to the URL.
    for cookie in cookie_jar.get_for_uri(&uri) {
        println!("Cookie set: {} = {}", cookie.name(), cookie.value());
    }

    // Send another request. The cookies previously set by the server will be
    // returned to it.
    let mut response = Request::get("http://httpbin.org/cookies")
        .cookie_jar(cookie_jar.clone())
        .body(())?
        .send()?;

    println!("Cookies received by server: {}", response.text()?);

    Ok(())
}
