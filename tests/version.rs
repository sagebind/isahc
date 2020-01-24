use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::config::VersionNegotiation;
use isahc::prelude::*;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "latest compatible negotiation politely asks for HTTP/2" {
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(contains(("upgrade", "h2c"))),
            ])
            .respond_with(status_code(200))
        );

        Request::get(server.url("/"))
            .version_negotiation(VersionNegotiation::latest_compatible())
            .body(())
            .unwrap()
            .send()
            .unwrap();

    }
}
