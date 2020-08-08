use isahc::prelude::*;
use std::{
    thread::sleep,
    time::Duration,
};
use testserver::endpoint;

/// Issue #3
#[test]
fn request_errors_if_read_timeout_is_reached() {
    // Spawn a slow server.
    let endpoint = endpoint! {
        body: |writer| {
            sleep(Duration::from_secs(1));
            writer.write_all(b"hello world")
        },
    };

    // Send a request with a timeout.
    let result = Request::post(endpoint.url())
        .timeout(Duration::from_millis(500))
        .body("hello world")
        .unwrap()
        .send();

    // Client should time-out.
    match result {
        Err(isahc::Error::Timeout) => {}
        e => {
            panic!("expected timeout error, got {:?}", e);
        }
    }
}
