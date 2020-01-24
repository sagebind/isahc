use httptest::{mappers::*, responders::status_code, Expectation};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "accept headers populated by default" {
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(all_of![
                    contains(("accept", "*/*")),
                    contains(("accept-encoding", "deflate, gzip")),
                ]),
            ])
            .respond_with(status_code(200))
        );

        isahc::get(server.url("/")).unwrap();
    }

    test "user agent contains expected format" {
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(
                    contains(("user-agent", matches(r"^curl/\S+ isahc/\S+$"))),
                ),
            ])
            .respond_with(status_code(200))
        );

        isahc::get(server.url("/")).unwrap();
    }
}
