use httptest::{mappers::*, responders::status_code, Expectation};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "returns correct response code" {
        for status in &[200u16, 202, 204, 302, 308, 400, 403, 404, 418, 429, 451, 500, 503] {
            server.expect(
                Expectation::matching(request::method_path("GET", "/"))
                .respond_with(status_code(*status))
            );

            let response = isahc::get(server.url("/")).unwrap();

            assert_eq!(response.status(), *status);
        }
    }
}
