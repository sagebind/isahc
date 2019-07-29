//! This example demonstrates the use of `send_async()` to make a request
//! asynchronously using the futures-based API.
#![cfg(feature = "nightly")]
#![feature(async_await)]

use chttp::prelude::*;

fn main() -> Result<(), chttp::Error> {
    futures::executor::block_on(async {
        let mut response = Request::get("http://example.org")
            .body(())?
            .send_async()
            .await?;

        println!("Status: {}", response.status());
        println!("Headers:\n{:?}", response.headers());
        println!("Body: {}", response.text_async().await?);

        Ok(())
    })
}
