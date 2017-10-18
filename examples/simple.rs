extern crate chttp;


fn main() {
    let mut response = chttp::get("https://example.org").unwrap();
    let body = response.body_mut().text().unwrap();
    println!("{:?}", response.headers());
    println!("{}", body);
}
