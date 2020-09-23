use isahc::prelude::*;
use mockito::{mock, server_url};

#[test]
fn get_request() {
    let m = mock("GET", "/").create();

    isahc::get(server_url()).unwrap();

    m.assert();
}

#[test]
fn head_request() {
    let m = mock("HEAD", "/").create();

    isahc::head(server_url()).unwrap();

    m.assert();
}

#[test]
fn post_request() {
    let m = mock("POST", "/").create();

    isahc::post(server_url(), ()).unwrap();

    m.assert();
}

#[test]
fn put_request() {
    let m = mock("PUT", "/").create();

    isahc::put(server_url(), ()).unwrap();

    m.assert();
}

#[test]
fn delete_request() {
    let m = mock("DELETE", "/").create();

    isahc::delete(server_url()).unwrap();

    m.assert();
}

#[test]
fn arbitrary_foobar_request() {
    let m = mock("FOOBAR", "/").create();

    Request::builder()
        .method("FOOBAR")
        .uri(server_url())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}
