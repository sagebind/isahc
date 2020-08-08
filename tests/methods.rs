use isahc::prelude::*;
use testserver::endpoint;

#[test]
fn get_request() {
    let endpoint = endpoint!();

    isahc::get(endpoint.url()).unwrap();

    assert_eq!(endpoint.request().method, "GET");
}

#[test]
fn head_request() {
    let endpoint = endpoint!();

    isahc::head(endpoint.url()).unwrap();

    assert_eq!(endpoint.request().method, "HEAD");
}

#[test]
fn post_request() {
    let endpoint = endpoint!();

    isahc::post(endpoint.url(), ()).unwrap();

    assert_eq!(endpoint.request().method, "POST");
}

#[test]
fn put_request() {
    let endpoint = endpoint!();

    isahc::put(endpoint.url(), ()).unwrap();

    assert_eq!(endpoint.request().method, "PUT");
}

#[test]
fn delete_request() {
    let endpoint = endpoint!();

    isahc::delete(endpoint.url()).unwrap();

    assert_eq!(endpoint.request().method, "DELETE");
}

#[test]
fn arbitrary_foobar_request() {
    let endpoint = endpoint!();

    Request::builder()
        .method("FOOBAR")
        .uri(endpoint.url())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(endpoint.request().method, "FOOBAR");
}
