use flate2::read::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::prelude::*;
use std::io::Read;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "gzip-encoded response is decoded automatically" {
        let body = "hello world";
        let mut body_encoded = Vec::new();

        GzEncoder::new(body.as_bytes(), Compression::default())
            .read_to_end(&mut body_encoded)
            .unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(contains(("accept-encoding", "deflate, gzip"))),
            ])
            .respond_with(status_code(200).insert_header("Content-Encoding", "gzip").body(body_encoded))
        );

        let mut response = isahc::get(server.url("/")).unwrap();

        assert_eq!(response.text().unwrap(), body);
    }

    test "deflate-encoded response is decoded automatically" {
        let body = "hello world";
        let mut body_encoded = Vec::new();

        DeflateEncoder::new(body.as_bytes(), Compression::default())
            .read_to_end(&mut body_encoded)
            .unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(contains(("accept-encoding", "deflate, gzip"))),
            ])
            .respond_with(status_code(200).insert_header("Content-Encoding", "deflate").body(body_encoded))
        );

        let mut response = isahc::get(server.url("/")).unwrap();

        assert_eq!(response.text().unwrap(), body);
    }

    test "content is decoded even if not listed as accepted" {
        let body = "hello world";
        let mut body_encoded = Vec::new();

        GzEncoder::new(body.as_bytes(), Compression::default())
            .read_to_end(&mut body_encoded)
            .unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/"),
                request::headers(contains(("accept-encoding", "deflate"))),
            ])
            .respond_with(status_code(200).insert_header("Content-Encoding", "gzip").body(body_encoded))
        );

        let mut response = Request::get(server.url("/"))
            .header("Accept-Encoding", "deflate")
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.text().unwrap(), body);
    }

    test "unknown Content-Encoding returns error" {
        server.expect(
            Expectation::matching(request::method_path("GET", "/"))
            .respond_with(status_code(200).insert_header("Content-Encoding", "foo").body("hello world"))
        );

        let result = Request::get(server.url("/"))
            .header("Accept-Encoding", "deflate")
            .body(())
            .unwrap()
            .send();

        match result {
            Err(isahc::Error::InvalidContentEncoding(_)) => {}
            _ => panic!("expected unknown encoding error, instead got {:?}", result),
        };
    }
}
