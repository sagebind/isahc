use isahc::prelude::*;
use mockito::{mock, server_url};
use std::thread::sleep;
use std::time::Duration;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    /// Issue #3
    test "request errors if read timeout is reached" {
        // Spawn a slow server.
        let m = mock("POST", "/")
            .with_body_from_fn(|_| {
                sleep(Duration::from_secs(1));
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
        match result {
            Err(isahc::Error::Timeout) => {}
            e => {
                panic!("expected timeout error, got {:?}", e);
            }
        }

        m.assert();
    }
}
