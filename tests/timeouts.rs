use isahc::prelude::*;
use std::{io::{self, Cursor, Read}, thread, time::Duration};
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
    assert!(matches!(result, Err(isahc::Error::Timeout)));

    assert_eq!(m.requests().len(), 1);
}

/// Issue #154
#[test]
fn timeout_during_response_body_produces_error() {
    struct SlowReader;

    impl Read for SlowReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            thread::sleep(Duration::from_secs(2));
            Ok(0)
        }
    }

    let m = mock! {
        body_reader: Cursor::new(vec![0; 100_000]).chain(SlowReader),
    };

    let mut response = Request::get(m.url())
        .timeout(Duration::from_millis(500))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    // Because of the short timeout, the response body should abort while being
    // read from.
    assert_eq!(response.copy_to(std::io::sink()).unwrap_err().kind(), std::io::ErrorKind::TimedOut);
}
