use isahc::prelude::*;
use mockito::{mock, server_url, Matcher};

#[test]
fn accept_headers_populated_by_default() {
    let m = mock("GET", "/")
        .match_header("accept", "*/*")
        .match_header("accept-encoding", "deflate, gzip")
        .create();

    isahc::get(server_url()).unwrap();

    m.assert();
}

#[test]
fn user_agent_contains_expected_format() {
    let m = mock("GET", "/")
        .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
        .create();

    isahc::get(server_url()).unwrap();

    m.assert();
}

// Issue [#209](https://github.com/sagebind/isahc/issues/209)
#[test]
fn setting_an_empty_header_sends_a_header_with_no_value() {
    let m = mock("GET", "/")
        .match_header("an-empty-header", "")
        .create();

    Request::get(server_url())
        .header("an-empty-header", "")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}

// Issue [#209](https://github.com/sagebind/isahc/issues/209)
#[test]
fn setting_a_blank_header_sends_a_header_with_no_value() {
    let m = mock("GET", "/")
        .match_header("an-empty-header", "")
        .create();

    Request::get(server_url())
        .header("an-empty-header", "    ")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}

// Issue [#190](https://github.com/sagebind/isahc/issues/190)
#[test]
fn override_client_default_user_agent() {
    let client = HttpClient::builder()
        .default_header("user-agent", "foo")
        .build()
        .unwrap();

    let m = mock("GET", "/")
        .match_header("user-agent", "foo")
        .create();

    client.get(server_url()).unwrap();

    m.assert();
}

// Issue [#205](https://github.com/sagebind/isahc/issues/205)
#[test]
fn set_title_case_headers_to_true() {
    let client = HttpClient::builder()
        .default_header("foo-BAR", "baz")
        .title_case_headers(true)
        .build()
        .unwrap();

    let m = testserver::Mock::default();

    client.get(m.url()).unwrap();

    assert_eq!(m.request().method, "GET");
    m.request().headers.iter()
        .find(|(key, value)| key == "Foo-Bar" && value == "baz")
        .expect("header not found");
}

#[test]
fn header_can_be_inserted_in_httpclient_builder() {
    let host_header = server_url().replace("http://", "");
    let m = mock("GET", "/")
        .match_header("host", host_header.as_ref())
        .match_header("accept", "*/*")
        .match_header("accept-encoding", "deflate, gzip")
        // .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
        .match_header("user-agent", Matcher::Any)
        .match_header("X-header", "some-value1")
        .create();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(server_url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();
    m.assert();
}

#[test]
fn headers_in_request_builder_must_override_headers_in_httpclient_builder() {
    let host_header = server_url().replace("http://", "");
    let m = mock("GET", "/")
        .match_header("host", host_header.as_ref())
        .match_header("accept", "*/*")
        .match_header("accept-encoding", "deflate, gzip")
        // .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
        .match_header("user-agent", Matcher::Any)
        .match_header("X-header", "some-value2")
        .create();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .header("X-header", "some-value2")
        .uri(server_url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();
    m.assert();
}

#[ignore]
#[test]
fn multiple_headers_with_same_key_can_be_inserted_in_httpclient_builder() {
    let host_header = server_url().replace("http://", "");
    let m = mock("GET", "/")
        .match_header("host", host_header.as_ref())
        .match_header("accept", "*/*")
        .match_header("accept-encoding", "deflate, gzip")
        // .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
        .match_header("user-agent", Matcher::Any)
        .match_header("X-header", "some-value1")
        .match_header("X-header", "some-value2")
        .create();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .default_header("X-header", "some-value2")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(server_url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();
    m.assert();
}

#[test]
fn headers_in_request_builder_must_override_multiple_headers_in_httpclient_builder() {
    let host_header = server_url().replace("http://", "");
    let m = mock("GET", "/")
        .match_header("host", host_header.as_ref())
        .match_header("accept", "*/*")
        .match_header("accept-encoding", "deflate, gzip")
        // .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
        .match_header("user-agent", Matcher::Any)
        .match_header("X-header", "some-value3")
        .create();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .default_header("X-header", "some-value2")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .header("X-header", "some-value3")
        .uri(server_url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();
    m.assert();
}
