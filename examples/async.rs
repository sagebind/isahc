//! This example demonstrates the use of `send_async()` (incubating) to make a
//! request asynchronously using the unstable futures API.
#![cfg(feature = "nightly")]
#![feature(async_await)]

use chttp::prelude::*;

fn main() -> Result<(), chttp::Error> {
    utilities::logging();

    futures::executor::block_on(async {
        let response = Request::get("http://example.org")
            .body(())?
            .send_async()
            .await?;

        println!("Status: {}", response.status());
        println!("Headers:\n{:?}", response.headers());

        Ok(())
    })
}
