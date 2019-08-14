use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
    let client = HttpClient::new();
    let mut response = client.get("https://example.org")?;

    println!("Status: {}", response.status());
    println!("Headers:\n{:?}", response.headers());

    print!("{}", response.text()?);

    Ok(())
}
