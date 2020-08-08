use isahc::prelude::*;
use testserver::endpoint;

#[test]
fn accept_headers_populated_by_default() {
    let endpoint = endpoint!();

    isahc::get(endpoint.url()).unwrap();

    assert_eq!(endpoint.request().method, "GET");
    endpoint.request().expect_header("accept", "*/*");
    endpoint.request().expect_header("accept-encoding", "deflate, gzip");
}

#[test]
fn user_agent_contains_expected_format() {
    let endpoint = endpoint!();

    isahc::get(endpoint.url()).unwrap();

    assert_eq!(endpoint.request().method, "GET");
    endpoint.request().expect_header_regex("user-agent", r"^curl/\S+ isahc/\S+$");
}

// Issue [#209](https://github.com/sagebind/isahc/issues/209)
#[test]
fn setting_an_empty_header_sends_a_header_with_no_value() {
    let endpoint = endpoint!();

    Request::get(endpoint.url())
        .header("an-empty-header", "")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    endpoint.request().expect_header("an-empty-header", "");
}

// Issue [#209](https://github.com/sagebind/isahc/issues/209)
#[test]
fn setting_a_blank_header_sends_a_header_with_no_value() {
    let endpoint = endpoint!();

    Request::get(endpoint.url())
        .header("an-empty-header", "    ")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    endpoint.request().expect_header("an-empty-header", "");
}

// Issue [#190](https://github.com/sagebind/isahc/issues/190)
#[test]
fn override_client_default_user_agent() {
    let client = HttpClient::builder()
        .default_header("user-agent", "foo")
        .build()
        .unwrap();

    let endpoint = endpoint!();

    client.get(endpoint.url()).unwrap();

    endpoint.request().expect_header("user-agent", "foo");
}

// Issue [#205](https://github.com/sagebind/isahc/issues/205)
#[test]
fn set_title_case_headers_to_true() {
    let client = HttpClient::builder()
        .default_header("foo-BAR", "baz")
        .title_case_headers(true)
        .build()
        .unwrap();

    let endpoint = endpoint!();

    client.get(endpoint.url()).unwrap();

    assert_eq!(endpoint.request().method, "GET");
    endpoint.request().headers.iter()
        .find(|(key, value)| key == "Foo-Bar" && value == "baz")
        .expect("header not found");
}

#[test]
fn header_can_be_inserted_in_httpclient_builder() {
    let endpoint = endpoint!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(endpoint.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    endpoint.request().expect_header("accept", "*/*");
    endpoint.request().expect_header("accept-encoding", "deflate, gzip");
    endpoint.request().expect_header_regex("user-agent",r"^curl/\S+ isahc/\S+$");
    endpoint.request().expect_header("X-header", "some-value1");
}

#[test]
fn headers_in_request_builder_must_override_headers_in_httpclient_builder() {
    let endpoint = endpoint!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .header("X-header", "some-value2")
        .uri(endpoint.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    endpoint.request().expect_header("accept", "*/*");
    endpoint.request().expect_header("accept-encoding", "deflate, gzip");
    endpoint.request().expect_header_regex("user-agent",r"^curl/\S+ isahc/\S+$");
    endpoint.request().expect_header("X-header", "some-value2");
}

#[test]
fn multiple_headers_with_same_key_can_be_inserted_in_httpclient_builder() {
    let endpoint = endpoint!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .default_header("X-header", "some-value2")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(endpoint.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    endpoint.request().expect_header("accept", "*/*");
    endpoint.request().expect_header("accept-encoding", "deflate, gzip");
    endpoint.request().expect_header_regex("user-agent",r"^curl/\S+ isahc/\S+$");
    endpoint.request().expect_header("X-header", "some-value1");
    endpoint.request().expect_header("X-header", "some-value2");
}

#[test]
fn headers_in_request_builder_must_override_multiple_headers_in_httpclient_builder() {
    let endpoint = endpoint!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .default_header("X-header", "some-value2")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .header("X-header", "some-value3")
        .uri(endpoint.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    endpoint.request().expect_header("accept", "*/*");
    endpoint.request().expect_header("accept-encoding", "deflate, gzip");
    endpoint.request().expect_header_regex("user-agent",r"^curl/\S+ isahc/\S+$");
    endpoint.request().expect_header("X-header", "some-value3");
}
