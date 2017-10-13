extern crate chttp;

use std::io::Read;


fn main() {
    let mut response = chttp::get("http://example.org");

    let mut body = String::new();
    match response.body_mut() {
        &mut chttp::Entity::Stream(ref mut stream) => {
            stream.read_to_string(&mut body);
        }
        _ => {}
    }

    println!("{}", body);
}
