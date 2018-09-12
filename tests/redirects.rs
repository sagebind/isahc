extern crate chttp;
extern crate env_logger;
extern crate rouille;

use chttp::http::Request;
use chttp::Options;

mod common;

#[test]
fn response_301_no_follow() {
    common::setup();

    let server = common::TestServer::spawn(|request| {
        match request.raw_url() {
            "/a" => rouille::Response::redirect_301("/b"),
            _ => rouille::Response::text("ok"),
        }
    });

    let response = chttp::get(&format!("{}/a", &server.endpoint())).unwrap();

    assert_eq!(response.status(), 301);
}

#[test]
fn response_301_auto_follow() {
    common::setup();

    let server = common::TestServer::spawn(|request| {
        match request.raw_url() {
            "/a" => rouille::Response::redirect_301("/b"),
            _ => rouille::Response::text("ok"),
        }
    });

    let mut response = Request::get(format!("{}/a", server.endpoint()))
        .extension(Options::default()
            .with_redirect_policy(chttp::options::RedirectPolicy::Follow))
        .body(())
        .map_err(Into::into)
        .and_then(chttp::send)
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.body_mut().text().unwrap(), "ok");
}
