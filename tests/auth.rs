use isahc::auth::*;
use isahc::prelude::*;
use mockito::{mock, server_url, Matcher};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "credentials without auth config does nothing" {
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

    test "basic auth sends authorization header" {
        let m = mock("GET", "/")
            .match_header("authorization", "Basic Y2xhcms6cXVlcnR5") // base64
            .create();

        Request::get(server_url())
            .authentication(Authentication::new().basic(true))
            .credentials(Credentials::new("clark", "querty"))
            .body(())
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }

    #[cfg(feature = "spnego")]
    test "negotiate auth exists" {
        let m = mock("GET", "/")
            .match_header("Authorization", Matcher::Missing)
            .with_status(401)
            .with_header("WWW-Authenticate", "Negotiate")
            .create();

        let response = Request::get(server_url())
            .authentication(Authentication::new().negotiate(true))
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 401);

        m.assert();
    }
}
