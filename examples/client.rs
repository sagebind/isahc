//! Another simple example that creates a custom HTTP client instance and sends
//! a GET request with it instead of using the default client.

use isahc::{
    config::RedirectPolicy,
    prelude::*,
};
use std::{
    io::{copy, stdout},
    time::Duration,
};

fn main() -> Result<(), isahc::Error> {
    // Create a custom client instance and customize a couple things different
    // than the default settings. Check the documentation of `HttpClient` and
    // `Configurable` for everything that can be customized.
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(5))
        .redirect_policy(RedirectPolicy::Follow)
        .build()?;

    let mut response = client.get("https://rust-lang.org")?;

    // Copy the response body directly to stdout.
    copy(response.body_mut(), &mut stdout())?;

    Ok(())
}
