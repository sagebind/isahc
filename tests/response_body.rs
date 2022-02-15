use futures_lite::{future::block_on, io::AsyncReadExt};
use isahc::prelude::*;
use std::{io, io::Read};
use testserver::mock;

#[macro_use]
mod utils;

#[test]
fn simple_response_body() {
    let m = mock! {
        body: "hello world",
    };

    let mut response = isahc::get(m.url()).unwrap();
    let response_text = response.text().unwrap();

    assert_eq!(response_text, "hello world");
}

#[test]
fn response_body_bytes() {
    let m = mock! {
        body: "hello world",
    };

    let mut response = isahc::get(m.url()).unwrap();
    let bytes = response.bytes().unwrap();

    assert_eq!(bytes, "hello world".as_bytes());
}

#[test]
fn response_body_bytes_async() {
    let m = mock! {
        body: "hello world",
    };

    block_on(async move {
        let mut response = isahc::get_async(m.url()).await.unwrap();
        let bytes = response.bytes().await.unwrap();

        assert_eq!(bytes, "hello world".as_bytes());
    });
}

#[test]
fn zero_length_response_body() {
    let m = mock! {
        body: "",
    };

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(response.body().len(), Some(0));
    assert!(!response.body().is_empty());
}

#[test]
fn large_response_body() {
    let body = "wow so large ".repeat(1000);

    let m = {
        let body = body.clone();
        mock! {
            body: body.clone(),
        }
    };

    let mut response = isahc::get(m.url()).unwrap();
    let response_text = response.text().unwrap();

    assert_eq!(response_text, body);
}

#[test]
fn response_body_with_content_length_knows_its_size() {
    let m = mock! {
        body: "hello world",
    };

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(response.body().len(), Some(11));
}

#[test]
fn response_body_with_chunked_encoding_has_unknown_size() {
    let m = mock! {
        body: "hello world",
        transfer_encoding: true,
    };

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(response.body().len(), None);
}

// See issue #341.
#[test]
fn head_request_with_content_length_response_returns_empty_body() {
    let m = mock! {
        headers {
            "content-length": 767,
        }
    };

    let response = isahc::head(m.url()).unwrap();

    assert!(response.body().is_empty());
}

// See issue #64.
#[test]
fn dropping_client_does_not_abort_response_transfer() {
    let body = "hello world\n".repeat(8192);
    let m = {
        let body = body.clone();
        mock! {
            body: body.clone(),
        }
    };

    let client = isahc::HttpClient::new().unwrap();
    let mut response = client.get(m.url()).unwrap();
    drop(client);

    assert_eq!(response.text().unwrap().len(), body.len());
}

// See issue #72.
#[test]
fn reading_from_response_body_after_eof_continues_to_return_eof() {
    let m = mock! {
        body: "hello world",
    };

    let mut response = isahc::get(m.url()).unwrap();
    let mut body = response.body_mut();

    // Read until EOF
    io::copy(&mut body, &mut io::sink()).unwrap();

    // Read after already receiving EOF
    let mut buf = [0; 1024];
    for _ in 0..3 {
        assert_eq!(body.read(&mut buf).unwrap(), 0);
    }
}

#[test]
fn consume_unread_response_body() {
    let body = "wow so large ".repeat(1000);

    let m = {
        let body = body.clone();
        mock! {
            _ => {
                body: body.clone(),
            },
        }
    };

    let mut response = isahc::get(m.url()).unwrap();
    response.consume().unwrap();

    let mut buf = [0; 8192];
    assert_matches!(response.body_mut().read(&mut buf), Ok(0));
}

#[test]
fn consume_unread_response_body_async() {
    let body = "wow so large ".repeat(1000);

    let m = {
        let body = body.clone();
        mock! {
            _ => {
                body: body.clone(),
            },
        }
    };

    block_on(async move {
        let mut response = isahc::get_async(m.url()).await.unwrap();
        response.consume().await.unwrap();

        let mut buf = [0; 8192];
        assert_matches!(response.body_mut().read(&mut buf).await, Ok(0));
    });
}
