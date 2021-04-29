//! Sample program that demonstrates how to upload a file using a `PUT` request.
//! Since 1.0, you can use a `File` as the request body directly, or anything
//! implementing `Read`, when using the synchronous APIs.
//!
//! If using the asynchronous APIs, you will need an asynchronous version of
//! `File` such as the one provided by [`async-fs`](https://docs.rs/async-fs).
//! Isahc does not provide an implementation for you.

use isahc::prelude::*;
use std::fs::File;

fn main() -> Result<(), isahc::Error> {
    // We're opening the source code file you are looking at right now for
    // reading.
    let file = File::open(file!())?;

    // Perform the upload.
    let mut response = isahc::put("https://httpbin.org/put", file)?;

    // Print interesting info from the response.
    println!("Status: {}", response.status());
    println!("Headers: {:#?}", response.headers());
    print!("{}", response.text()?);

    Ok(())
}
