use isahc::{
    config::RedirectPolicy,
    prelude::*,
};
use testserver::mock;

#[test]
fn response_301_no_follow() {
    let m = mock! {
        status: 301,
        headers {
            "Location": "/2",
        }
    };

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(response.status(), 301);
    assert_eq!(response.headers()["Location"], "/2");
    assert_eq!(response.effective_uri().unwrap().path(), "/");

    assert!(!m.requests().is_empty());
}

#[test]
fn response_301_auto_follow() {
    let m2 = mock! {
        status: 200,
        body: "ok",
    };
    let location = m2.url();

    let m1 = mock! {
        status: 301,
        headers {
            "Location": location,
        }
    };

    let mut response = Request::get(m1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().unwrap(), "ok");
    assert_eq!(response.effective_uri().unwrap().to_string(), m2.url());

    assert!(!m1.requests().is_empty());
    assert!(!m2.requests().is_empty());
}

#[test]
fn headers_are_reset_every_redirect() {
    let m2 = mock! {
        status: 200,
        headers {
            "X-Foo": "bbb",
            "X-Baz": "zzz",
        }
    };
    let location = m2.url();

    let m1 = mock! {
        status: 301,
        headers {
            "Location": location,
            "X-Foo": "aaa",
            "X-Bar": "zzz",
        }
    };

    let response = Request::get(m1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["X-Foo"], "bbb");
    assert_eq!(response.headers()["X-Baz"], "zzz");
    assert!(!response.headers().contains_key("X-Bar"));

    assert!(!m1.requests().is_empty());
    assert!(!m2.requests().is_empty());
}

#[test]
fn _303_redirect_changes_post_to_get() {
    let m2 = mock!();
    let location = m2.url();

    let m1 = mock! {
        status: 303,
        headers {
            "Location": location,
        }
    };

    let response = Request::post(m1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.effective_uri().unwrap().to_string(), m2.url());

    assert_eq!(m1.request().method, "POST");
    assert_eq!(m2.request().method, "GET");
}

#[test]
fn redirect_limit_is_respected() {
    let m = mock! {
        status: 301,
        headers {
            "Location": "/next",
        }
    };

    let result = Request::get(m.url())
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
    assert_eq!(m.requests().len(), 6);
}

#[test]
fn auto_referer_sets_expected_header() {
    let m3 = mock!();

    let m2 = {
        let location = m3.url();
        mock! {
            status: 301,
            headers {
                "Location": location,
            }
        }
    };

    let m1 = {
        let location = m2.url();
        mock! {
            status: 301,
            headers {
                "Location": location,
            }
        }
    };

    Request::get(m1.url())
        .redirect_policy(RedirectPolicy::Follow)
        .auto_referer()
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m2.request().expect_header("Referer", m1.url());
    m3.request().expect_header("Referer", m2.url());
}
