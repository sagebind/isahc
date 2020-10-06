use isahc::{
    auth::*,
    prelude::*,
};
use testserver::mock;

#[test]
fn credentials_without_auth_config_does_nothing() {
    let m = mock!();

    Request::get(m.url())
        .credentials(Credentials::new("clark", "querty"))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.request().get_header("authorization").count(), 0);
}

#[test]
fn basic_auth_sends_authorization_header() {
    let m = mock!();

    Request::get(m.url())
        .authentication(Authentication::basic())
        .credentials(Credentials::new("clark", "querty"))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    // base64
    m.request().expect_header("authorization", "Basic Y2xhcms6cXVlcnR5");
}

#[cfg(feature = "spnego")]
#[test]
fn negotiate_auth_exists() {
    let m = mock! {
        status: 401,
        headers {
            "WWW-Authenticate": "Negotiate",
        }
    };

    Request::get(m.url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert!(!m.requests().is_empty());
}

#[cfg(all(feature = "spnego", windows))]
#[test]
fn negotiate_on_windows_provides_a_token() {
    let m = mock! {
        status: 200,
        headers {
            "WWW-Authenticate": "Negotiate",
        }
    };

    let response = Request::get(m.url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    m.request().expect_header_regex("authorization", r"Negotiate \w+=*");
}
