use flate2::Compression;
use flate2::read::{DeflateEncoder, GzEncoder};
use isahc::prelude::*;
use mockito::{mock, server_url};
use std::io::Read;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "gzip-encoded response is decoded automatically" {
        let body = "hello world";
        let mut body_encoded = Vec::new();

        GzEncoder::new(body.as_bytes(), Compression::default())
            .read_to_end(&mut body_encoded)
            .unwrap();

        let m = mock("GET", "/")
            .match_header("Accept-Encoding", "gzip, deflate")
            .with_header("Content-Encoding", "gzip")
            .with_body(&body_encoded)
            .create();

        let mut response = isahc::get(server_url()).unwrap();

        assert_eq!(response.text().unwrap(), body);
        m.assert();
    }

    test "deflate-encoded response is decoded automatically" {
        let body = "hello world";
        let mut body_encoded = Vec::new();

        DeflateEncoder::new(body.as_bytes(), Compression::default())
            .read_to_end(&mut body_encoded)
            .unwrap();

        let m = mock("GET", "/")
            .match_header("Accept-Encoding", "gzip, deflate")
            .with_header("Content-Encoding", "deflate")
            .with_body(&body_encoded)
            .create();

        let mut response = isahc::get(server_url()).unwrap();

        assert_eq!(response.text().unwrap(), body);
        m.assert();
    }

    test "content is decoded even if not listed as accepted" {
        let body = "hello world";
        let mut body_encoded = Vec::new();

        GzEncoder::new(body.as_bytes(), Compression::default())
            .read_to_end(&mut body_encoded)
            .unwrap();

        let m = mock("GET", "/")
            .match_header("Accept-Encoding", "deflate")
            .with_header("Content-Encoding", "gzip")
            .with_body(&body_encoded)
            .create();

        let mut response = Request::get(server_url())
            .header("Accept-Encoding", "deflate")
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.text().unwrap(), body);
        m.assert();
    }

    test "unknown Content-Encoding returns error" {
        let m = mock("GET", "/")
            .with_header("Content-Encoding", "foo")
            .with_body("hello world")
            .create();

        let result = Request::get(server_url())
            .header("Accept-Encoding", "deflate")
            .body(())
            .unwrap()
            .send();

        match result {
            Err(isahc::Error::InvalidContentEncoding(_)) => {}
            _ => panic!("expected unknown encoding error, instead got {:?}", result),
        };

        m.assert();
    }
}
