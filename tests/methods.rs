use chttp::prelude::*;
use mockito::{mock, server_url};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "GET request" {
        let m = mock("GET", "/").create();

        chttp::get(server_url()).unwrap();

        m.assert();
    }

    test "HEAD request" {
        let m = mock("HEAD", "/").create();

        chttp::head(server_url()).unwrap();

        m.assert();
    }

    test "POST request" {
        let m = mock("POST", "/").create();

        chttp::post(server_url(), ()).unwrap();

        m.assert();
    }

    test "PUT request" {
        let m = mock("PUT", "/").create();

        chttp::put(server_url(), ()).unwrap();

        m.assert();
    }

    test "DELETE request" {
        let m = mock("DELETE", "/").create();

        chttp::delete(server_url()).unwrap();

        m.assert();
    }

    test "arbitrary FOOBAR request" {
        let m = mock("FOOBAR", "/").create();

        Request::builder()
            .method("FOOBAR")
            .uri(server_url())
            .body(())
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }
}
