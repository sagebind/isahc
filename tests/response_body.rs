use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::prelude::*;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "simple response body" {
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).body("hello world"))
        );

        let mut response = isahc::get(server.url("/")).unwrap();
        let response_text = response.text().unwrap();

        assert_eq!(response_text, "hello world");
    }

    test "large response body" {
        let body = "wow so large ".repeat(1000);
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).body(body.clone()))
        );

        let mut response = isahc::get(server.url("/")).unwrap();
        let response_text = response.text().unwrap();

        assert_eq!(response_text, body);
    }

    test "response body with Content-Length knows its size" {
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).body("hello world"))
        );

        let response = isahc::get(server.url("/")).unwrap();

        assert_eq!(response.body().len(), Some(11));
    }

    test "response body with chunked encoding has unknown size" {
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).insert_header("Transfer-Encoding", "chunked").body("5\r\nhello\r\n0\r\n\r\n"))
        );

        let response = isahc::get(server.url("/")).unwrap();

        assert_eq!(response.body().len(), None);
    }

    // See issue #64.
    test "dropping client does not abort response transfer" {
        let body = "hello world\n".repeat(8192);
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).body(body.clone()))
        );

        let client = isahc::HttpClient::new().unwrap();
        let mut response = client.get(server.url("/")).unwrap();
        drop(client);

        assert_eq!(response.text().unwrap().len(), body.len());
    }

    // See issue #72.
    test "reading from response body after EOF continues to return EOF" {
        use std::{io, io::Read};

        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).body("hello world"))
        );

        let mut response = isahc::get(server.url("/")).unwrap();
        let mut body = response.body_mut();

        // Read until EOF
        io::copy(&mut body, &mut io::sink()).unwrap();

        // Read after already receiving EOF
        let mut buf = [0; 1024];
        for _ in 0..3 {
            assert_eq!(body.read(&mut buf).unwrap(), 0);
        }
    }
}
