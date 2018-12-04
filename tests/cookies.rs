extern crate chttp;
extern crate env_logger;
extern crate rouille;

mod common;

use chttp::middleware::Middleware;

#[test]
fn cookie_lifecycle() {
    common::setup();

    let uri: chttp::http::Uri = "https://example.com/foo".parse().unwrap();
    let jar = chttp::cookies::CookieJar::default();

    jar.filter_response(chttp::http::Response::builder()
        .header(chttp::http::header::SET_COOKIE, "foo=bar")
        .header(chttp::http::header::SET_COOKIE, "baz=123")
        .extension(uri.clone())
        .body(chttp::Body::default())
        .unwrap());

    let request = jar.filter_request(chttp::http::Request::builder()
        .uri(uri)
        .body(chttp::Body::default())
        .unwrap());

    assert_eq!(request.headers()[chttp::http::header::COOKIE], "baz=123; foo=bar");
}
