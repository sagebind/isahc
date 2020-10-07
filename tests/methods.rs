use isahc::prelude::*;
use testserver::mock;

#[test]
fn get_request() {
    let m = mock!();

    isahc::get(m.url()).unwrap();

    assert_eq!(m.request().method, "GET");
}

#[test]
fn head_request() {
    let m = mock!();

    isahc::head(m.url()).unwrap();

    assert_eq!(m.request().method, "HEAD");
}

#[test]
fn post_request() {
    let m = mock!();

    isahc::post(m.url(), ()).unwrap();

    assert_eq!(m.request().method, "POST");
}

#[test]
fn put_request() {
    let m = mock!();

    isahc::put(m.url(), ()).unwrap();

    assert_eq!(m.request().method, "PUT");
}

#[test]
fn delete_request() {
    let m = mock!();

    isahc::delete(m.url()).unwrap();

    assert_eq!(m.request().method, "DELETE");
}

#[test]
fn arbitrary_foobar_request() {
    let m = mock!();

    Request::builder()
        .method("FOOBAR")
        .uri(m.url())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.request().method, "FOOBAR");
}
