use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::config::RedirectPolicy;
use isahc::prelude::*;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "response 301 no follow" {
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/"),
            ])
            .respond_with(status_code(301).insert_header("Location", "/2"))
        );

        let response = isahc::get(server.url("/")).unwrap();

        assert_eq!(response.status(), 301);
        assert_eq!(response.headers()["Location"], "/2");
        assert_eq!(response.effective_uri().unwrap().path(), "/");
    }

    test "response 301 auto follow" {
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/"),
            ])
            .respond_with(status_code(301).insert_header("Location", "/2"))
        );
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/2"),
            ])
            .respond_with(status_code(200).body("ok"))
        );

        let mut response = Request::get(server.url("/"))
            .redirect_policy(RedirectPolicy::Follow)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.text().unwrap(), "ok");
        assert_eq!(response.effective_uri().unwrap().path(), "/2");
    }

    test "headers are reset every redirect" {
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/"),
            ])
            .respond_with(status_code(301).insert_header("Location", "/b").insert_header("X-Foo", "aaa").insert_header("X-Bar", "zzz"))
        );
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/b"),
            ])
            .respond_with(status_code(200).insert_header("X-Foo", "bbb").insert_header("X-Baz", "zzz"))
        );

        let response = Request::get(server.url("/"))
            .redirect_policy(RedirectPolicy::Follow)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["X-Foo"], "bbb");
        assert_eq!(response.headers()["X-Baz"], "zzz");
        assert!(!response.headers().contains_key("X-Bar"));
    }

    test "303 redirect changes POST to GET" {
        server.expect(
            Expectation::matching(all_of![
                request::method("POST"),
                request::path("/"),
            ])
            .respond_with(status_code(303).insert_header("Location", "/2"))
        );
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/2"),
            ])
            .respond_with(status_code(200))
        );

        let response = Request::post(server.url("/"))
            .redirect_policy(RedirectPolicy::Follow)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(response.effective_uri().unwrap().path(), "/2");
    }

    test "redirect limit is respected" {
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/"),
            ])
            .times(3)
            .respond_with(status_code(301).insert_header("Location", "/2"))
        );
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/2"),
            ])
            .times(3)
            .respond_with(status_code(301).insert_header("Location", "/"))
        );

        let result = Request::get(server.url("/"))
            .redirect_policy(RedirectPolicy::Limit(5))
            .body(())
            .unwrap()
            .send();

        // Request should error with too many redirects.
        assert!(match result {
            Err(isahc::Error::TooManyRedirects) => true,
            _ => false,
        });
    }

    test "auto referer sets expected header" {
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/a"),
            ])
            .respond_with(status_code(301).insert_header("Location", "/b"))
        );
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/b"),
                request::headers(contains(
                    ("referer", server.url_str("/a")),
                )),
            ])
            .respond_with(status_code(301).insert_header("Location", "/c"))
        );
        server.expect(
            Expectation::matching(all_of![
                request::method("GET"),
                request::path("/c"),
                request::headers(contains(
                    ("referer", server.url_str("/b")),
                )),
            ])
            .respond_with(status_code(200))
        );

        Request::get(server.url("/a"))
            .redirect_policy(RedirectPolicy::Follow)
            .auto_referer()
            .body(())
            .unwrap()
            .send()
            .unwrap();
    }
}
