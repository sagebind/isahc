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
    let endpoint = endpoint!(
        status_code: 401,
        headers {
            "WWW-Authenticate": "Negotiate",
        }
    );

    Request::get(endpoint.url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(endpoint.requests().len(), 1);
}

#[cfg(all(feature = "spnego", windows))]
#[test]
fn negotiate_on_windows_provides_a_token() {
    let endpoint = endpoint!(
        status_code: 200,
        headers {
            "WWW-Authenticate": "Negotiate",
        }
    );

    let response = Request::get(server_url())
        .authentication(Authentication::negotiate())
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(endpoint.requests().len(), 1);
    endpoint.request().expect_header("Authorization", r"Negotiate \w+=*"); // base64
}
