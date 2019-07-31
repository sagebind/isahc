use mockito::{mock, server_url};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "returns correct response code" {
        for status in [200u16, 202, 204, 302, 308, 400, 403, 404, 418, 429, 451, 500, 503].iter() {
            let m = mock("GET", "/")
                .with_status(*status as usize)
                .create();

            let response = isahc::get(server_url()).unwrap();

            assert_eq!(response.status(), *status);
            m.assert();
        }
    }
}
