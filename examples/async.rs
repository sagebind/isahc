//! This example demonstrates the use of `send_async()` to make a request
//! asynchronously using the futures-based API.
#![cfg(feature = "nightly")]
#![feature(async_await)]

use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
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
