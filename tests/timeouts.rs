use isahc::prelude::*;
use mockito::{mock, server_url};
use std::thread::sleep;
use std::time::Duration;

/// Issue #3
#[test]
fn request_errors_if_read_timeout_is_reached() {
    // Spawn a slow server.
    let m = mock("POST", "/")
        .with_body_from_fn(|_| {
            sleep(Duration::from_secs(1));
            Ok(())
        })
        .create();

    // Send a request with a timeout.
    let result = Request::post(server_url())
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

    m.assert();
}
