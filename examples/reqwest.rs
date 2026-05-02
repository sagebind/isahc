fn main() {
    let body = reqwest::blocking::get("https://example.org")
        .unwrap()
        .text()
        .unwrap();
    println!("{}", body);
}
