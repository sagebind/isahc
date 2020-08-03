use flate2::read::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use isahc::prelude::*;
use mockito::{mock, server_url};
use std::io::Read;

#[test]
fn gzip_encoded_response_is_decoded_automatically() {
    let body = "hello world";
    let mut body_encoded = Vec::new();

    GzEncoder::new(body.as_bytes(), Compression::default())
        .read_to_end(&mut body_encoded)
        .unwrap();

    let m = mock("GET", "/")
        .match_header("Accept-Encoding", "deflate, gzip")
        .with_header("Content-Encoding", "gzip")
        .with_body(&body_encoded)
        .create();

    let mut response = isahc::get(server_url()).unwrap();

    assert_eq!(response.text().unwrap(), body);
    m.assert();
}

#[test]
fn deflate_encoded_response_is_decoded_automatically() {
    let body = "hello world";
    let mut body_encoded = Vec::new();

    DeflateEncoder::new(body.as_bytes(), Compression::default())
        .read_to_end(&mut body_encoded)
        .unwrap();

    let m = mock("GET", "/")
        .match_header("Accept-Encoding", "deflate, gzip")
        .with_header("Content-Encoding", "deflate")
        .with_body(&body_encoded)
        .create();

    let mut response = isahc::get(server_url()).unwrap();

    assert_eq!(response.text().unwrap(), body);
    m.assert();
}

#[test]
fn content_is_decoded_even_if_not_listed_as_accepted() {
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

#[test]
fn unknown_content_encoding_returns_error() {
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
