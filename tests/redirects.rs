use isahc::config::RedirectPolicy;
use isahc::prelude::*;
use testserver::endpoint;

#[test]
fn response_301_no_follow() {
    let endpoint = endpoint! {
        status_code: 301,
        headers {
            "location": "/foo",
        }
    };

    let response = isahc::get(endpoint.url()).unwrap();

    assert_eq!(response.status(), 301);
    assert_eq!(response.headers()["Location"], "/foo");
    assert_eq!(response.effective_uri().unwrap().to_string(), endpoint.url());
}

#[test]
fn response_301_auto_follow() {
    let endpoint2 = endpoint! {
        status_code: 200,
        body: "ok",
    };

    let endpoint1 = endpoint! {
        status_code: 301,
        headers {
            "location": endpoint2.url(),
        }
    };

    let mut response = Request::get(endpoint1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().unwrap(), "ok");
    assert_eq!(response.effective_uri().unwrap().to_string(), endpoint2.url());

    assert_eq!(endpoint1.request().method, "GET");
    assert_eq!(endpoint2.request().method, "GET");
}

#[test]
fn headers_are_reset_every_redirect() {
    let endpoint1 = endpoint! {
        headers {
            "X-Foo": "bbb",
            "X-Baz": "zzz",
        }
    };

    let endpoint2 = endpoint! {
        status_code: 301,
        headers {
            "Location": endpoint1.url(),
            "X-Foo": "aaa",
            "X-Bar": "zzz",
        }
    };

    let response = Request::get(endpoint2.url())
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

#[test]
fn a_303_redirect_changes_post_to_get() {
    let endpoint2 = endpoint!();
    let endpoint1 = endpoint! {
        status_code: 303,
        headers {
            "Location": endpoint2.url(),
        }
    };

    let response = Request::post(endpoint1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.effective_uri().unwrap().to_string(), endpoint2.url());

    assert_eq!(endpoint1.request().method, "POST");
    assert_eq!(endpoint2.request().method, "GET");
}

#[test]
fn redirect_limit_is_respected() {
    let endpoint3 = endpoint!();
    let endpoint2 = endpoint! {
        status_code: 301,
        headers {
            "Location": endpoint3.url(),
        }
    };
    let endpoint1 = endpoint! {
        status_code: 301,
        headers {
            "Location": endpoint2.url(),
        }
    };

    let result = Request::get(endpoint1.url())
        .redirect_policy(RedirectPolicy::Limit(1))
        .body(())
        .unwrap()
        .send();

    // Request should error with too many redirects.
    assert!(match result {
        Err(isahc::Error::TooManyRedirects) => true,
        _ => false,
    });

    // After request (limit + 1) that returns a redirect should error.
    assert_eq!(endpoint1.requests().len(), 1);
    assert_eq!(endpoint2.requests().len(), 1);
    assert_eq!(endpoint3.requests().len(), 0);
}

#[test]
fn auto_referer_sets_expected_header() {
    let endpoint2 = endpoint!();
    let endpoint1 = endpoint! {
        status_code: 301,
        headers {
            "location": endpoint2.url(),
        }
    };

    Request::get(endpoint1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .auto_referer()
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(endpoint1.requests().len(), 1);
    endpoint2.request().expect_header("Referer", endpoint1.url());
}
