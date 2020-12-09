use flate2::{
    read::{DeflateEncoder, GzEncoder},
    Compression,
};
use isahc::prelude::*;
use std::io::Read;
use testserver::mock;

#[test]
fn gzip_encoded_response_is_decoded_automatically() {
    let body = "hello world";
    let mut body_encoded = Vec::new();

    GzEncoder::new(body.as_bytes(), Compression::default())
        .read_to_end(&mut body_encoded)
        .unwrap();

    let m = mock! {
        headers {
            "Content-Encoding": "gzip",
        }
        body: body_encoded.clone(),
    };

    let mut response = isahc::get(m.url()).unwrap();

    assert_eq!(response.text().unwrap(), body);
    m.request().expect_header("Accept-Encoding", "deflate, gzip");

    // Response body size should be unknown, because the actual content is
    // gzipped.
    assert_eq!(response.body().len(), None);
}

#[test]
fn request_gzip_without_automatic_decompression() {
    let body = "hello world";
    let mut body_encoded = Vec::new();

    GzEncoder::new(body.as_bytes(), Compression::default())
        .read_to_end(&mut body_encoded)
        .unwrap();

    let m = {
        let body_encoded = body_encoded.clone();
        mock! {
            headers {
                "Content-Encoding": "gzip",
            }
            body: body_encoded.clone(),
        }
    };

    let mut response = Request::get(m.url())
        .header("Accept-Encoding", "gzip")
        .automatic_decompression(false)
        .body(())
        .unwrap()
        .send()
        .unwrap();
    let mut body_received = Vec::new();
    response.body_mut().read_to_end(&mut body_received).unwrap();

    assert_eq!(body_received, body_encoded);
    m.request().expect_header("Accept-Encoding", "gzip");

    // Response body size should be known.
    assert_eq!(response.body().len(), Some(31));
}

#[test]
fn deflate_encoded_response_is_decoded_automatically() {
    let body = "hello world";
    let mut body_encoded = Vec::new();

    DeflateEncoder::new(body.as_bytes(), Compression::default())
        .read_to_end(&mut body_encoded)
        .unwrap();

    let m = mock! {
        headers {
            "Content-Encoding": "deflate",
        }
        body: body_encoded.clone(),
    };

    let mut response = isahc::get(m.url()).unwrap();

    assert_eq!(response.text().unwrap(), body);
    m.request().expect_header("Accept-Encoding", "deflate, gzip");

    // Response body size should be unknown, because the actual content is
    // compressed.
    assert_eq!(response.body().len(), None);
}

#[test]
fn content_is_decoded_even_if_not_listed_as_accepted() {
    let body = "hello world";
    let mut body_encoded = Vec::new();

    GzEncoder::new(body.as_bytes(), Compression::default())
        .read_to_end(&mut body_encoded)
        .unwrap();

    let m = mock! {
        headers {
            "Content-Encoding": "gzip",
        }
        body: body_encoded.clone(),
    };

    let mut response = Request::get(m.url())
        .header("Accept-Encoding", "deflate")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.text().unwrap(), body);
    m.request().expect_header("Accept-Encoding", "deflate");
}

#[test]
fn unknown_content_encoding_returns_error() {
    let m = mock! {
        headers {
            "Content-Encoding": "foo",
        }
        body: "hello world",
    };

    let result = Request::get(m.url())
        .header("Accept-Encoding", "deflate")
        .body(())
        .unwrap()
        .send();

    match result {
        Err(isahc::Error::InvalidContentEncoding(_)) => {}
        _ => panic!("expected unknown encoding error, instead got {:?}", result),
    };

    m.request().expect_header("Accept-Encoding", "deflate");
}
