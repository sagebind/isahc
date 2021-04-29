#![cfg(feature = "json")]

use futures_lite::{future::block_on, io::AsyncRead};
use isahc::prelude::*;
use serde_json::Value;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use testserver::mock;

#[macro_use]
mod utils;

#[test]
fn deserialize_json() {
    let m = mock! {
        body: r#"{
            "foo": "bar"
        }"#,
    };

    let mut response = isahc::get(m.url()).unwrap();
    let data = response.json::<Value>().unwrap();

    assert_eq!(data["foo"], "bar");
}

#[test]
fn deserialize_json_async() {
    let m = mock! {
        body: r#"{
            "foo": "bar"
        }"#,
    };

    block_on(async move {
        let mut response = isahc::get_async(m.url()).await.unwrap();
        let data = response.json::<Value>().await.unwrap();

        assert_eq!(data["foo"], "bar");
    });
}

#[test]
fn deserialize_json_async_io_error() {
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

    block_on(async move {
        let mut response = http::Response::new(BadReader);

        assert_matches!(response.json::<Value>().await, Err(e) if e.is_io());
    });
}
