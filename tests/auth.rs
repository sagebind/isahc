use isahc::auth::*;
use isahc::prelude::*;
use mockito::{mock, server_url, Matcher};

#[test]
fn credentials_without_auth_config_does_nothing() {
    let m = mock("GET", "/")
        .match_header("authorization", Matcher::Missing)
        .create();

    Request::get(server_url())
        .credentials(Credentials::new("clark", "querty"))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}

#[test]
fn basic_auth_sends_authorization_header() {
    let m = mock("GET", "/")
        .match_header("authorization", "Basic Y2xhcms6cXVlcnR5") // base64
        .create();

    Request::get(server_url())
        .authentication(Authentication::basic())
        .credentials(Credentials::new("clark", "querty"))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}

#[cfg(feature = "spnego")]
#[test]
fn negotiate_auth_exists() {
    let m = mock("GET", "/")
        .with_status(401)
        .with_header("WWW-Authenticate", "Negotiate")
        .create();

    Request::get(server_url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}

#[cfg(all(feature = "spnego", windows))]
#[test]
fn negotiate_on_windows_provides_a_token() {
    let m = mock("GET", "/")
        .match_header("Authorization", Matcher::Regex(r"Negotiate \w+=*".into()))
        .with_status(200)
        .with_header("WWW-Authenticate", "Negotiate")
        .create();

    let response = Request::get(server_url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);

    m.assert();
}
