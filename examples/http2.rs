extern crate chttp;


fn main() {
    let mut options = chttp::Options::default();
    options.preferred_http_version = Some(chttp::http::Version::HTTP_2);

    let client = chttp::Client::builder()
        .options(options)
        .build();

    let mut response = client.get("https://nghttp2.org").unwrap();
    let body = response.body_mut().text().unwrap();
    println!("{}", body);
}
