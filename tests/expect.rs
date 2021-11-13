use isahc::{Body, Request, prelude::*};
use testserver::mock;

#[test]
fn expect_header_is_sent_by_default() {
    let m = mock!();

    let body = Body::from_reader("hello world".as_bytes());

    isahc::post(m.url(), body).unwrap();

    m.request().expect_header("expect", "100-continue");
}

#[test]
fn expect_header_is_not_sent_when_disabled() {
    let m = mock!();

    let body = Body::from_reader("hello world".as_bytes());

    Request::post(m.url())
        .expect_continue(false)
        .body(body)
        .unwrap()
        .send()
        .unwrap();

    assert!(m.request().get_header("expect").next().is_none());
}
