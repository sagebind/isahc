use isahc::prelude::*;
use std::fs::File;

fn main() -> Result<(), isahc::Error> {
    let file = File::open(file!())?;
    let mut response = isahc::put("https://httpbin.org/put", file)?;

    println!("Status: {}", response.status());
    println!("Headers: {:#?}", response.headers());
    print!("{}", response.text()?);

    Ok(())
}
