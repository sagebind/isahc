use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::auth::*;
use isahc::prelude::*;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "credentials without auth config does nothing" {
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(not(contains(key("authorization")))),
            ])
            .respond_with(status_code(200))
        );

        Request::get(server.url("/"))
            .credentials(Credentials::new("clark", "querty"))
            .body(())
            .unwrap()
            .send()
            .unwrap();

    }

    test "basic auth sends authorization header" {
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(contains(("authorization", "Basic Y2xhcms6cXVlcnR5"))),
            ])
            .respond_with(status_code(200))
        );

        Request::get(server.url("/"))
            .authentication(Authentication::basic())
            .credentials(Credentials::new("clark", "querty"))
            .body(())
            .unwrap()
            .send()
            .unwrap();

    }

    #[cfg(feature = "spnego")]
    test "negotiate auth exists" {
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(401).insert_header("WWW-Authenticate", "Negotiate"))
        );

        Request::get(server.url("/"))
            .authentication(Authentication::negotiate())
            .body(())
            .unwrap()
            .send()
            .unwrap();

    }

    #[cfg(all(feature = "spnego", windows))]
    test "negotiate on windows provides a token" {
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(contains(("Authorization", matching(r"Negotiate \w=*")))),
            ])
            .respond_with(status_code(200).insert_header("WWW-Authenticate", "Negotiate"))
        );

        let response = Request::get(server.url("/"))
            .authentication(Authentication::negotiate())
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
    }
}
