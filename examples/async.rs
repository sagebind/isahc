//! This example demonstrates the use of `send_async()` to make a request
//! asynchronously using the futures-based API.

use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
    futures::executor::block_on(async {
        let mut response = isahc::get_async("http://example.org").await?;

        println!("Status: {}", response.status());
        println!("Headers:\n{:?}", response.headers());
        println!("Body: {}", response.text_async().await?);

        Ok(())
    })
}
