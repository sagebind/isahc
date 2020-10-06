use isahc::prelude::*;
use std::time::Duration;
use testserver::mock;

/// Issue #3
#[test]
fn request_errors_if_read_timeout_is_reached() {
    // Spawn a slow server.
    let m = mock! {
        delay: 1s,
    };

    // Send a request with a timeout.
    let result = Request::post(m.url())
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

    assert_eq!(m.requests().len(), 1);
}
