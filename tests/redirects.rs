use chttp::config::RedirectPolicy;
use chttp::prelude::*;
use mockito::{mock, server_url};

mod utils;

speculate::speculate! {
    before {
        utils::logging();
    }

    test "response 301 no follow" {
        let m = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/b")
            .create();

        let response = chttp::get(server_url()).unwrap();

        assert_eq!(response.status(), 301);
        assert_eq!(response.headers()["Location"], "/b");
        m.assert();
    }

    test "response 301 auto follow" {
        let m1 = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/b")
            .create();

        let m2 = mock("GET", "/b")
            .with_status(200)
            .with_body("ok")
            .create();

        let mut response = Request::get(server_url())
            .redirect_policy(RedirectPolicy::Follow)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.text().unwrap(), "ok");

        m1.assert();
        m2.assert();
    }

    test "redirect limit is respected" {
        let m1 = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/b")
            .create();

        let m2 = mock("GET", "/b")
            .with_status(301)
            .with_header("Location", "/")
            .create();

        let result = Request::get(server_url())
            .redirect_policy(RedirectPolicy::Limit(5))
            .body(())
            .unwrap()
            .send();

        // Request should error with too many redirects.
        assert!(match result {
            Err(chttp::Error::TooManyRedirects) => true,
            _ => false,
        });

        // After request (limit + 1) that returns a redirect should error.
        m1.expect(3);
        m2.expect(3);
    }
}
