use httptest::{
    mappers::*,
    responders::status_code,
    Expectation,
};
use isahc::prelude::*;
use std::{io, thread, time::Duration};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "metrics are disabled by default" {
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200))
        );

        let response = isahc::get(server.url("/")).unwrap();

        assert!(response.metrics().is_none());
    }

    test "enabling metrics causes metrics to be collected" {
        server.expect(
            Expectation::matching(request::method_path("POST", "/"))
            .respond_with(|| {
                thread::sleep(Duration::from_millis(10));
                status_code(200).body("hello world")
            })
        );

        let client = isahc::HttpClient::builder()
            .metrics(true)
            .build()
            .unwrap();

        let mut response = client.send(Request::post(server.url("/"))
            .body("hello server")
            .unwrap())
            .unwrap();

        let metrics = response.metrics().unwrap().clone();

        assert_eq!(metrics.upload_progress(), (12, 12));

        io::copy(response.body_mut(), &mut io::sink()).unwrap();

        assert_eq!(metrics.download_progress().0, 11);
        assert!(metrics.total_time() > Duration::default());
    }
}
