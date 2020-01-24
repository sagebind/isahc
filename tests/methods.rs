use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::prelude::*;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "GET request" {
        server.expect(
            Expectation::matching(request::method("GET"))
            .respond_with(status_code(200))
        );

        isahc::get(server.url("/")).unwrap();
    }

    test "HEAD request" {
        server.expect(
            Expectation::matching(request::method("HEAD"))
            .respond_with(status_code(200))
        );

        isahc::head(server.url("/")).unwrap();
    }

    test "POST request" {
        server.expect(
            Expectation::matching(request::method("POST"))
            .respond_with(status_code(200))
        );

        isahc::post(server.url("/"), ()).unwrap();
    }

    test "PUT request" {
        server.expect(
            Expectation::matching(request::method("PUT"))
            .respond_with(status_code(200))
        );

        isahc::put(server.url("/"), ()).unwrap();
    }

    test "DELETE request" {
        server.expect(
            Expectation::matching(request::method("DELETE"))
            .respond_with(status_code(200))
        );

        isahc::delete(server.url("/")).unwrap();
    }

    test "arbitrary FOOBAR request" {
        server.expect(
            Expectation::matching(request::method("FOOBAR"))
            .respond_with(status_code(200))
        );

        Request::builder()
            .method("FOOBAR")
            .uri(server.url("/"))
            .body(())
            .unwrap()
            .send()
            .unwrap();
    }
}
