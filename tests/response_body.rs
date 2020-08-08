use isahc::prelude::*;
use std::io::Read;
use testserver::endpoint;

#[test]
fn simple_response_body() {
    let endpoint = endpoint! {
        body: "hello world",
    };

    let mut response = isahc::get(endpoint.url()).unwrap();
    let response_text = response.text().unwrap();

    assert_eq!(response_text, "hello world");
    assert_eq!(endpoint.requests().len(), 1);
}

#[test]
fn large_response_body() {
    let body = "wow so large ".repeat(1000);

    let endpoint = endpoint! {
        body: body.clone(),
    };

    let mut response = isahc::get(endpoint.url()).unwrap();
    let response_text = response.text().unwrap();

    assert_eq!(response_text, body);
    assert_eq!(endpoint.requests().len(), 1);
}

#[test]
fn response_body_with_content_length_knows_its_size() {
    let endpoint = endpoint! {
        body: "hello world",
    };

    let response = isahc::get(endpoint.url()).unwrap();

    assert_eq!(response.body().len(), Some(11));
    assert_eq!(endpoint.requests().len(), 1);
}

#[test]
fn response_body_with_chunked_encoding_has_unknown_size() {
    let endpoint = endpoint! {
        body: |writer| {
            writer.write_all(b"hello world")
        },
    };

    let mut response = isahc::get(endpoint.url()).unwrap();

    assert_eq!(response.body().len(), None);

    let mut buf = Vec::new();
    response.body_mut().read_to_end(&mut buf).unwrap();

    assert_eq!(buf, b"hello world");
    assert_eq!(endpoint.requests().len(), 1);
}

// See issue #64.
#[test]
fn dropping_client_does_not_abort_response_transfer() {
    let body = "hello world\n".repeat(8192);

    let endpoint = endpoint! {
        body: body.clone(),
    };

    let client = isahc::HttpClient::new().unwrap();
    let mut response = client.get(endpoint.url()).unwrap();
    drop(client);

    assert_eq!(response.text().unwrap().len(), body.len());
    assert_eq!(endpoint.requests().len(), 1);
}

// See issue #72.
#[test]
fn reading_from_response_body_after_eof_continues_to_return_eof() {
    use std::{io, io::Read};

    let endpoint = endpoint! {
        body: "hello world",
    };

    let mut response = isahc::get(endpoint.url()).unwrap();
    let mut body = response.body_mut();

    // Read until EOF
    io::copy(&mut body, &mut io::sink()).unwrap();
    assert_eq!(endpoint.requests().len(), 1);

    // Read after already receiving EOF
    let mut buf = [0; 1024];
    for _ in 0..3 {
        assert_eq!(body.read(&mut buf).unwrap(), 0);
    }
}
