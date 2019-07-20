use chttp::prelude::*;
use mockito::{mock, server_url};
use std::thread::sleep;
use std::time::Duration;

mod utils;

speculate::speculate! {
    before {
        utils::logging();
    }

    /// Issue #3
    test "request errors if read timeout is reached" {
        // Spawn a slow server.
        let m = mock("POST", "/")
            .with_body_from_fn(|_| {
                sleep(Duration::from_millis(500));
                Ok(())
            })
            .create();

        // Send a request with a timeout.
        let result = Request::post(server_url())
            .timeout(Duration::from_millis(100))
            .body("hello world")
            .unwrap()
            .send();

        // Client should time-out.
        assert!(match result {
            Err(chttp::Error::Timeout) => true,
            _ => false,
        });

        m.assert();
    }
}
