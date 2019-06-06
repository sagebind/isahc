use std::env;

fn main() -> Result<(), chttp::Error> {
    env::set_var("RUST_BACKTRACE", "1");
    env::set_var("RUST_LOG", "trace");
    utilities::logging();

    let client = chttp::Client::new()?;
    let mut response = client.get("http://example.org")?;

    println!("Status: {}", response.status());
    println!("Headers:\n{:?}", response.headers());

    // Copy the response body directly to stdout.
    std::io::copy(response.body_mut(), &mut std::io::stdout())?;

    Ok(())
}
