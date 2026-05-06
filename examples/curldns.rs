use std::time::Duration;

use curl::{
    easy::{Easy, SslOpt},
    multi::Multi,
};

fn main() -> Result<(), curl::Error> {
    let mut curl = Easy::new();
    curl.url("https://example.com/")?;
    curl.ssl_options(SslOpt::new().native_ca(true))?;
    curl.verbose(true)?;

    let multi = Multi::new();
    let e = multi.add(curl).unwrap();

    while multi.perform().unwrap() > 0 {
        multi.messages(|message| {
            dbg!(message);
        });
        multi.wait(&mut [], Duration::from_secs(1)).unwrap();
    }

    drop(e);

    Ok(())
}
