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
        let response_text = response.body_mut().text().unwrap();

        assert_eq!(response_text, "hello world");
        m.assert();
    }

    test "large response body" {
        let body = "wow so large ".repeat(1000);

        let m = mock("GET", "/")
            .with_body(&body)
            .create();

        let mut response = isahc::get(server_url()).unwrap();
        let response_text = response.body_mut().text().unwrap();

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
}
