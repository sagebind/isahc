use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
    let client = HttpClient::builder()
        .danger_allow_unsafe_ssl(true)
        .build()?;
    let mut response = client.get("https://www.iep.utm.edu/desert/")?;

    println!("Status: {}", response.status());
    println!("Headers:\n{:?}", response.headers());

    // Copy the response body directly to stdout.
    // std::io::copy(response.body_mut(), &mut std::io::stdout())?;
    print!("{}", response.text()?);

    Ok(())
}
