extern crate chttp;
extern crate env_logger;
extern crate rouille;

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

    // Create an impatient client.
    let mut options = chttp::Options::default();
    options.timeout = Some(Duration::from_secs(2));
    let client = chttp::Client::builder().options(options).build().unwrap();

    // Send a request.
    let result = client.post(&server.endpoint(), "hello world");

    // Client should time-out.
    assert!(match result {
        Err(chttp::Error::Timeout) => true,
        _ => false,
    });
}
