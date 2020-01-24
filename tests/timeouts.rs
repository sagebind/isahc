use httptest::{
    mappers::*,
    responders::status_code,
    Expectation,
};
use isahc::prelude::*;
use std::time::Duration;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    /// Issue #3
    test "request errors if read timeout is reached" {
        let (_tx, rx) = crossbeam_channel::bounded::<()>(0);
        // Spawn a slow server.
        server.expect(
            Expectation::matching(request::method_path("POST", "/"))
            .respond_with(move || {
                // block to mimic a long running request.
                let _ = rx.recv();
                status_code(200)
            })
        );

        // Send a request with a timeout.
        let result = Request::post(server.url("/"))
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
    }
}
