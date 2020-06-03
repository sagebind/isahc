//! Capturing log events using a [log] implementation.
//!
//! [log]: https://github.com/rust-lang/log

fn main() -> Result<(), isahc::Error> {
    env_logger::init();

    let mut response = isahc::get("https://example.org")?;

    // Consume the response stream quietly.
    std::io::copy(response.body_mut(), &mut std::io::sink())?;

    Ok(())
}
