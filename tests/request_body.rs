use httptest::{mappers::*, responders::status_code, Expectation};
use isahc::prelude::*;
use isahc::Body;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
        let server = httptest::Server::run();
    }

    test "request with body of known size" {
        //for method in &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "FOOBAR"] {
        for method in &["GET"] {
            let body = "MyVariableOne=ValueOne&MyVariableTwo=ValueTwo";

            server.expect(
                Expectation::matching(all_of![
                    request::method_path(*method, "/"),
                    request::headers(contains(("content-type", "application/x-www-form-urlencoded"))),
                    request::body(body),
                ])
                .respond_with(status_code(200))
            );

            Request::builder()
                .method(*method)
                .uri(server.url("/"))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body)
                .unwrap()
                .send()
                .unwrap();
        }
    }

    test "request with body of unknown size uses chunked encoding" {
        for method in &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "FOOBAR"] {
            let body = "foo";

            server.expect(
                Expectation::matching(all_of![
                    request::method_path(*method, "/"),
                    request::headers(contains(("transfer-encoding", "chunked"))),
                    request::body(body),
                ])
                .respond_with(status_code(200))
            );
            Request::builder()
                .method(*method)
                .uri(server.url("/"))
                // This header should be ignored
                .header("transfer-encoding", "identity")
                .body(Body::from_reader(body.as_bytes()))
                .unwrap()
                .send()
                .unwrap();
        }
    }

    // test "Content-Length header takes precedence over body object's length" {
    //     for method in &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "FOOBAR"] {
    //        server.expect(
    //            Expectation::matching(all_of![
    //                request::method_path(*method, "/"),
    //                request::headers(contains(("content-length", "3"))),
    //                request::body("abc"),
    //            ])
    //            .respond_with(status_code(200))
    //        );
    //         Request::builder()
    //             .method(*method)
    //             .uri(server.url("/"))
    //             // Override given body's length
    //             .header("content-length", "3")
    //             .body("abc123")
    //             .unwrap()
    //             .send()
    //             .unwrap();
    //     }
    // }
}
