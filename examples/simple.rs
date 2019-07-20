use chttp::prelude::*;

fn main() -> Result<(), chttp::Error> {
    let client = HttpClient::new();
    let mut response = client.get("http://example.org")?;

    println!("Status: {}", response.status());
    println!("Headers:\n{:?}", response.headers());

    // Copy the response body directly to stdout.
    // std::io::copy(response.body_mut(), &mut std::io::stdout())?;
    print!("{}", response.text()?);

    Ok(())
}
