use std::{collections::HashMap, io::Cursor};

use isahc::{Body, Request};

fn main() -> Result<(), isahc::Error> {
    let mut default_headers = HashMap::new();
    default_headers.insert("Foo".to_string(), "bar".to_string());

    let client = isahc::HttpClient::builder()
        .default_headers(default_headers)
        .build()?;

    Ok(())
}
