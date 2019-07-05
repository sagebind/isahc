use chttp::config::RedirectPolicy;
use chttp::prelude::*;
use utilities::rouille;

mod utilities;

#[test]
fn response_301_no_follow() {
    utilities::logging();

    let server = utilities::server::spawn(|request| match request.raw_url() {
        "/a" => rouille::Response::redirect_301("/b"),
        _ => rouille::Response::text("ok"),
    });

    let response = chttp::get(&format!("{}/a", &server.endpoint())).unwrap();

    assert_eq!(response.status(), 301);
}

#[test]
fn response_301_auto_follow() {
    utilities::logging();

    let server = utilities::server::spawn(|request| match request.raw_url() {
        "/a" => rouille::Response::redirect_301("/b"),
        _ => rouille::Response::text("ok"),
    });

    let mut response = Request::get(format!("{}/a", server.endpoint()))
        .redirect_policy(RedirectPolicy::Follow)
        .body(())
        .map_err(Into::into)
        .and_then(chttp::send)
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().unwrap(), "ok");
}

#[test]
fn redirect_limit_is_respected() {
    utilities::logging();

    let server = utilities::server::spawn(|request| {
        let count = request.raw_url()[1..].parse::<u32>().unwrap();

        rouille::Response::redirect_301(format!("/{}", count + 1))
    });

    let result = Request::get(format!("{}/0", server.endpoint()))
        .redirect_policy(RedirectPolicy::Limit(5))
        .body(())
        .map_err(Into::into)
        .and_then(chttp::send);

    // Request should error with too many redirects.
    assert!(match result {
        Err(chttp::Error::TooManyRedirects) => true,
        _ => false,
    });
}
