use isahc::prelude::*;
use isahc::Body;
use mockito::{mock, server_url};

#[test]
fn request_with_body_of_known_size() {
    for method in &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "FOOBAR"] {
        let body = "MyVariableOne=ValueOne&MyVariableTwo=ValueTwo";

        let m = mock(method, "/")
            .match_header("content-type", "application/x-www-form-urlencoded")
            .match_body(body)
            .create();

        Request::builder()
            .method(*method)
            .uri(server_url())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }
}

#[test]
fn request_with_body_of_unknown_size_uses_chunked_encoding() {
    for method in &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "FOOBAR"] {
        let body = "foo";

        let m = mock(method, "/")
            .match_header("transfer-encoding", "chunked")
            .match_body(body)
            .create();

        Request::builder()
            .method(*method)
            .uri(server_url())
            // This header should be ignored
            .header("transfer-encoding", "identity")
            .body(Body::from_reader(body.as_bytes()))
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }
}

#[test]
#[ignore]
fn content_length_header_takes_precedence_over_body_objects_length() {
    for method in &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "FOOBAR"] {
        let m = mock(method, "/")
            .match_header("content-length", "3")
            .match_body("abc") // Truncated to 3 bytes
            .create();

        Request::builder()
            .method(*method)
            .uri(server_url())
            // Override given body's length
            .header("content-length", "3")
            .body("abc123")
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }
}
