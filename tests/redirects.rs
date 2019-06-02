#![feature(async_await)]

use chttp::http::Request;
use chttp::Options;
use futures::executor::block_on;

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

    block_on(async {
        assert_eq!(response.status(), 200);
        assert_eq!(response.body_mut().text().await.unwrap(), "ok");
    })
}

#[test]
fn redirect_limit_is_respected() {
    common::setup();

    let server = common::TestServer::spawn(|request| {
        let count = request.raw_url()[1..].parse::<u32>().unwrap();

        rouille::Response::redirect_301(format!("/{}", count + 1))
    });

    let result = Request::get(format!("{}/0", server.endpoint()))
        .extension(Options::default()
            .with_redirect_policy(chttp::options::RedirectPolicy::Limit(5)))
        .body(())
        .map_err(Into::into)
        .and_then(chttp::send);


    // Request should error with too many redirects.
    assert!(match result {
        Err(chttp::Error::TooManyRedirects) => true,
        _ => false,
    });
}
