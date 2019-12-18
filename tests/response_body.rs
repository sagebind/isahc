use isahc::prelude::*;
use mockito::{mock, server_url};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "simple response body" {
        let m = mock("GET", "/")
            .with_body("hello world")
            .create();

        let mut response = isahc::get(server_url()).unwrap();
        let response_text = response.text().unwrap();

        assert_eq!(response_text, "hello world");
        m.assert();
    }

    test "large response body" {
        let body = "wow so large ".repeat(1000);

        let m = mock("GET", "/")
            .with_body(&body)
            .create();

        let mut response = isahc::get(server_url()).unwrap();
        let response_text = response.text().unwrap();

        assert_eq!(response_text, body);
        m.assert();
    }

    test "response body with Content-Length knows its size" {
        let m = mock("GET", "/")
            .with_body("hello world")
            .create();

        let response = isahc::get(server_url()).unwrap();

        assert_eq!(response.body().len(), Some(11));
        m.assert();
    }

    test "response body with chunked encoding has unknown size" {
        let m = mock("GET", "/")
            .with_body_from_fn(|w| w.write_all(b"hello world"))
            .create();

        let response = isahc::get(server_url()).unwrap();

        assert_eq!(response.body().len(), None);
        m.assert();
    }

    // See issue #64.
    test "dropping client does not abort response transfer" {
        let body = "hello world\n".repeat(8192);
        let m = mock("GET", "/")
            .with_body(&body)
            .create();

        let client = isahc::HttpClient::new().unwrap();
        let mut response = client.get(server_url()).unwrap();
        drop(client);

        assert_eq!(response.text().unwrap().len(), body.len());
        m.assert();
    }

    // See issue #72.
    test "reading from response body after EOF continues to return EOF" {
        use std::{io, io::Read};

        let m = mock("GET", "/")
            .with_body("hello world")
            .create();

        let mut response = isahc::get(server_url()).unwrap();
        let mut body = response.body_mut();

        // Read until EOF
        io::copy(&mut body, &mut io::sink()).unwrap();
        m.assert();

        // Read after already receiving EOF
        let mut buf = [0; 1024];
        for _ in 0..3 {
            assert_eq!(body.read(&mut buf).unwrap(), 0);
        }
    }
}
