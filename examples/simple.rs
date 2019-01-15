fn main() -> Result<(), chttp::Error> {
    env_logger::init();

    let mut response = chttp::get("http://example.org")?;

    println!("Status: {}", response.status());
    println!("Headers:\n{:?}", response.headers());

    // Copy the response body directly to stdout.
    std::io::copy(response.body_mut(), &mut std::io::stdout())?;

    Ok(())
}
