//! Capturing log events using a [tracing] subscriber.
//!
//! [tracing]: https://github.com/tokio-rs/tracing

fn main() -> Result<(), isahc::Error> {
    tracing_subscriber::fmt::init();

    let mut response = isahc::get("https://example.org")?;

    // Consume the response stream quietly.
    std::io::copy(response.body_mut(), &mut std::io::sink())?;

    Ok(())
}
