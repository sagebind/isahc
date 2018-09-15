extern crate chttp;
extern crate env_logger;

fn main() {
    ::std::env::set_var("RUST_LOG", "chttp=trace,curl=trace");
    env_logger::init();

    let mut response = chttp::get("http://example.org").unwrap();
    let body = response.body_mut().text().unwrap();

    println!("{:?}", response.headers());
    println!("{}", body);
}
