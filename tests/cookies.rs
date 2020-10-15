#![cfg(feature = "cookies")]

use isahc::{cookies::CookieJar, HttpClient};
use testserver::mock;

#[test]
fn cookie_lifecycle() {
    let client = HttpClient::builder().cookies().build().unwrap();

    let m1 = mock! {
        headers {
            "set-cookie": "foo=bar",
            "set-cookie": "baz=123",
        }
    };
    let m2 = mock!();

    let response1 = client.get(m1.url()).unwrap();

    assert!(response1.extensions().get::<CookieJar>().is_some());

    let response2 = client.get(m2.url()).unwrap();

    assert!(response2.extensions().get::<CookieJar>().is_some());

    m2.request().expect_header("cookie", "baz=123; foo=bar");
}
