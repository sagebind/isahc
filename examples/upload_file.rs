use isahc::prelude::*;
use std::fs::File;

fn main() -> Result<(), isahc::Error> {
    env_logger::init();
    let client = HttpClient::new()?;

    let file = File::open(file!())?;
    let request = isahc::http::Request::put("https://httpbin.org/put")
        .body(file)?;

    let mut response = client.send_sync(request)?;

    println!("Status: {}", response.status());
    println!("Headers: {:#?}", response.headers());
    print!("{}", response.text()?);

    Ok(())
}
