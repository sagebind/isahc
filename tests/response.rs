#![feature(async_await)]

use futures::executor::block_on;

mod common;

#[test]
fn simple_response_body() {
    common::setup();

    let server = common::TestServer::spawn(|_| {
        rouille::Response::text("hello world")
    });

    block_on(async {
        let mut response = chttp::get(server.endpoint()).unwrap();
        let response_text = response.body_mut().text().await.unwrap();
        assert_eq!(response_text, "hello world");
    })
}

#[test]
fn large_response_body() {
    common::setup();

    let server = common::TestServer::spawn(|_| {
        rouille::Response::text("wow so large ".repeat(1000))
    });

    block_on(async {
        let mut response = chttp::get(server.endpoint()).unwrap();
        let response_text = response.body_mut().text().await.unwrap();
        assert_eq!(response_text, "wow so large ".repeat(1000));
    })
}
