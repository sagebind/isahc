use isahc::prelude::*;
use mockito::{mock, server_url, Matcher};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "accept headers populated by default" {
        let m = mock("GET", "/")
            .match_header("accept", "*/*")
            .match_header("accept-encoding", "deflate, gzip")
            .create();

        isahc::get(server_url()).unwrap();

        m.assert();
    }

    test "user agent contains expected format" {
        let m = mock("GET", "/")
            .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
            .create();

        isahc::get(server_url()).unwrap();

        m.assert();
    }

    test "header can be inserted in HttpClient::builder" {

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

    test "headers in Request::builder must override headers in HttpClient::builder" {

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

    // test "multiple headers with same key can be inserted in HttpClient::builder" {

    //     let host_header = server_url().replace("http://", "");
    //     let m = mock("GET", "/")
    //         .match_header("host", host_header.as_ref())
    //         .match_header("accept", "*/*")
    //         .match_header("accept-encoding", "deflate, gzip")
    //         // .match_header("user-agent", Matcher::Regex(r"^curl/\S+ isahc/\S+$".into()))
    //         .match_header("user-agent", Matcher::Any)
    //         .match_header("X-header", "some-value1")
    //         .match_header("X-header", "some-value2")
    //         .create();

    //     let client = HttpClient::builder()
    //        .default_header("X-header", "some-value1")
    //        .default_header("X-header", "some-value2")
    //        .build()
    //        .unwrap();

    //     let request = Request::builder()
    //        .method("GET")
    //        .uri(server_url())
    //        .body(())
    //        .unwrap();

    //     let _ = client.send(request).unwrap();
    //     m.assert();
    // }




    test "headers in Request::builder must override multiple headers in HttpClient::builder" {

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
}
