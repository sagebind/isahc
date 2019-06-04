//! This example demonstrates the use of `send_async()` (incubating) to make a
//! request asynchronously using the unstable futures API.
use chttp::Client;
use chttp::http::Request;
use futures::executor;
use futures::prelude::*;

fn main() -> Result<(), chttp::Error> {
    env_logger::init();
    let client = Client::new()?;

    let future = client
        .send_async(Request::get("http://example.org").body(())?)
        .map(|response| {
            println!("Status: {}", response.status());
            println!("Headers:\n{:?}", response.headers());

            response
        });

    executor::block_on(future)?;

    Ok(())
}
