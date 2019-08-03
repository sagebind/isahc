use isahc::config::RedirectPolicy;
use isahc::prelude::*;
use mockito::{mock, server_url};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "response 301 no follow" {
        let m = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/2")
            .create();

        let response = isahc::get(server_url()).unwrap();

        assert_eq!(response.status(), 301);
        assert_eq!(response.headers()["Location"], "/2");
        m.assert();
    }

    test "response 301 auto follow" {
        let m1 = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/2")
            .create();

        let m2 = mock("GET", "/2")
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

    test "headers are reset every redirect" {
        let m1 = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/b")
            .with_header("X-Foo", "aaa")
            .with_header("X-Bar", "zzz")
            .create();

        let m2 = mock("GET", "/b")
            .with_header("X-Foo", "bbb")
            .with_header("X-Baz", "zzz")
            .create();

        let response = Request::get(server_url())
            .redirect_policy(RedirectPolicy::Follow)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["X-Foo"], "bbb");
        assert_eq!(response.headers()["X-Baz"], "zzz");
        assert!(!response.headers().contains_key("X-Bar"));

        m1.assert();
        m2.assert();
    }

    test "303 redirect changes POST to GET" {
        let m1 = mock("POST", "/")
            .with_status(303)
            .with_header("Location", "/2")
            .create();

        let m2 = mock("GET", "/2").create();

        let response = Request::post(server_url())
            .redirect_policy(RedirectPolicy::Follow)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
        m1.assert();
        m2.assert();
    }

    test "redirect limit is respected" {
        let m1 = mock("GET", "/")
            .with_status(301)
            .with_header("Location", "/2")
            .create();

        let m2 = mock("GET", "/2")
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
            Err(isahc::Error::TooManyRedirects) => true,
            _ => false,
        });

        // After request (limit + 1) that returns a redirect should error.
        m1.expect(3);
        m2.expect(3);
    }
}
