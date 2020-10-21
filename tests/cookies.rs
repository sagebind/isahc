#![cfg(feature = "cookies")]

use isahc::{cookies::CookieJar, prelude::*};
use testserver::mock;

#[test]
fn cookie_lifecycle() {
    let jar = CookieJar::default();
    let client = HttpClient::builder().cookie_jar(jar.clone()).build().unwrap();

    let m1 = mock! {
        headers {
            "set-cookie": "foo=bar",
            "set-cookie": "baz=123",
        }
    };
    let m2 = mock!();

    let response1 = client.get(m1.url()).unwrap();

    assert!(response1.cookie_jar().is_some());

    let response2 = client.get(m2.url()).unwrap();

    assert!(response2.cookie_jar().is_some());

    dbg!(m2.request()).expect_header("cookie", "baz=123; foo=bar");
}
