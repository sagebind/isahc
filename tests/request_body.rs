use futures_lite::{future::block_on, AsyncRead};
use isahc::{prelude::*, AsyncBody, Body, Request};
use std::{
    error::Error,
    io::{self, Read},
    pin::Pin,
    task::{Context, Poll},
};
use test_case::test_case;
use testserver::mock;

#[macro_use]
mod utils;

#[test_case("GET")]
#[test_case("HEAD")]
#[test_case("POST")]
#[test_case("PUT")]
#[test_case("DELETE")]
#[test_case("PATCH")]
#[test_case("FOOBAR")]
fn request_with_body_of_known_size(method: &str) {
    let body = "MyVariableOne=ValueOne&MyVariableTwo=ValueTwo";

    let m = mock!();

    Request::builder()
        .method(method)
        .uri(m.url())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.request().method(), method);
    m.request()
        .expect_header("content-length", body.len().to_string());
    m.request()
        .expect_header("content-type", "application/x-www-form-urlencoded");
    m.request().expect_body(body);
}

#[test_case("GET")]
#[test_case("HEAD")]
#[test_case("POST")]
#[test_case("PUT")]
#[test_case("DELETE")]
#[test_case("PATCH")]
#[test_case("FOOBAR")]
fn request_with_body_of_unknown_size_uses_chunked_encoding(method: &str) {
    let body = "foo";

    let m = mock!();

    Request::builder()
        .method(method)
        .uri(m.url())
        // This header should be ignored
        .header("transfer-encoding", "identity")
        .body(Body::from_reader(body.as_bytes()))
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.request().method(), method);
    m.request().expect_header("transfer-encoding", "chunked");
    m.request().expect_body(body);
}

#[test_case("GET")]
#[test_case("HEAD")]
#[test_case("POST")]
#[test_case("PUT")]
#[test_case("DELETE")]
#[test_case("PATCH")]
#[test_case("FOOBAR")]
fn content_length_header_takes_precedence_over_body_objects_length(method: &str) {
    let m = mock!();

    Request::builder()
        .method(method)
        .uri(m.url())
        // Override given body's length
        .header("content-length", "3")
        .body("abc123")
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.request().method(), method);
    m.request().expect_header("content-length", "3");
    m.request().expect_body("abc"); // truncated to 3 bytes
}

#[test]
fn upload_from_bad_reader_returns_error_with_original_cause() {
    let m = mock!();

    struct BadReader;

    impl Read for BadReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::UnexpectedEof.into())
        }
    }

    let result = isahc::put(m.url(), Body::from_reader(BadReader));

    assert_matches!(&result, Err(e) if e.kind() == isahc::error::ErrorKind::Io);
    assert_eq!(
        result
            .unwrap_err()
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn upload_from_bad_async_reader_returns_error_with_original_cause() {
    let m = mock!();

    struct BadReader;

    impl AsyncRead for BadReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
        }
    }

    let result =
        block_on(async { isahc::put_async(m.url(), AsyncBody::from_reader(BadReader)).await });

    assert_matches!(&result, Err(e) if e.kind() == isahc::error::ErrorKind::Io);
    assert_eq!(
        result
            .unwrap_err()
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
}
