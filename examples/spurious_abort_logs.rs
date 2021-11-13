use std::{error::Error, io, time::Duration};

use isahc::{
    config::{Configurable, RedirectPolicy},
    Request,
    RequestExt,
};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    read_url("https://tynan.com/feed/")?;

    Ok(())
}

pub fn read_url(url: &str) -> Result<Vec<u8>, String> {
    let client = match Request::get(url)
        .timeout(Duration::from_secs(5))
        .header("User-Agent", "el_monitorro/0.1.0")
        .redirect_policy(RedirectPolicy::Limit(10))
        .body(())
    {
        Ok(cl) => cl,
        Err(er) => {
            let msg = format!("{:?}", er);

            return Err(msg);
        }
    };

    match client.send() {
        Ok(mut response) => {
            let mut writer: Vec<u8> = vec![];

            if let Err(err) = io::copy(response.body_mut(), &mut writer) {
                let msg = format!("{:?}", err);

                return Err(msg);
            }

            Ok(writer)
        }
        Err(error) => {
            let msg = format!("{:?}", error);

            Err(msg)
        }
    }
}
