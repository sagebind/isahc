extern crate chttp;
extern crate env_logger;
extern crate rouille;

use chttp::http::Request;
use chttp::Options;
use std::time::Duration;
use std::thread;

mod common;

/// Issue #3
#[test]
fn request_errors_if_read_timeout_is_reached() {
    common::setup();

    // Spawn a slow server.
    let server = common::TestServer::spawn(|_| {
        thread::sleep(Duration::from_secs(3));
        rouille::Response::text("hello world")
    });

    // Send a request with a timeout.
    let result = Request::post(server.endpoint())
        .extension(Options::default()
            .with_timeout(Some(Duration::from_secs(2))))
        .body("hello world")
        .map_err(Into::into)
        .and_then(chttp::send);

    // Client should time-out.
    assert!(match result {
        Err(chttp::Error::Timeout) => true,
        _ => false,
    });
}
