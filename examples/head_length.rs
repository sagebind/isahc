use std::io::Cursor;

use isahc::{Body, Request};

fn main() -> Result<(), isahc::Error> {
    let weird_request = Request::head("http://download.opensuse.org/update/tumbleweed/repodata/repomd.xml")
        .body(Body::from_reader(Cursor::new(b"")))?;

    let error = isahc::send(weird_request).unwrap_err();
    eprintln!("{}", error);

    Ok(())
}
