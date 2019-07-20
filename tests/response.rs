use mockito::{mock, server_url};

mod utils;

speculate::speculate! {
    before {
        utils::logging();
    }

    test "simple response body" {
        let mock = mock("GET", "/")
            .with_body("hello world")
            .create();

        let mut response = chttp::get(server_url()).unwrap();
        let response_text = response.body_mut().text().unwrap();
        assert_eq!(response_text, "hello world");

        mock.assert();
    }

    test "large response body" {
        let body = "wow so large ".repeat(1000);

        let mock = mock("GET", "/")
            .with_body(&body)
            .create();

        let mut response = chttp::get(server_url()).unwrap();
        let response_text = response.body_mut().text().unwrap();
        assert_eq!(response_text, body);

        mock.assert();
    }
}
