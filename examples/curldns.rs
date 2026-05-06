use curl::easy::Easy;

fn main() -> Result<(), curl::Error> {
    let mut curl = Easy::new();

    curl.url("https://example.com/")?;

    curl.perform()?;

    Ok(())
}
