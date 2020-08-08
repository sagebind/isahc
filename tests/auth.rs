use isahc::auth::*;
use isahc::prelude::*;
use testserver::endpoint;

#[test]
fn credentials_without_auth_config_does_nothing() {
    let endpoint = endpoint!();

    Request::get(endpoint.url())
        .credentials(Credentials::new("clark", "querty"))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(endpoint.request().get_header("authorization").count(), 0);
}

#[test]
fn basic_auth_sends_authorization_header() {
    let endpoint = endpoint!();

    Request::get(endpoint.url())
        .authentication(Authentication::basic())
        .credentials(Credentials::new("clark", "querty"))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    endpoint.request().expect_header("authorization", "Basic Y2xhcms6cXVlcnR5"); // base64
}

#[cfg(feature = "spnego")]
#[test]
fn negotiate_auth_exists() {
    let m = mockito::mock("GET", "/")
        .with_status(401)
        .with_header("WWW-Authenticate", "Negotiate")
        .create();

    Request::get(mockito::server_url())
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
    let m = mockito::mock("GET", "/")
        .match_header("Authorization", mockito::Matcher::Regex(r"Negotiate \w+=*".into()))
        .with_status(200)
        .with_header("WWW-Authenticate", "Negotiate")
        .create();

    let response = Request::get(mockito::server_url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);

    m.assert();
}
