#![cfg(feature = "unstable-interceptors")]

use isahc::HttpClient;
use mockito::{mock, server_url};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "change HTTP method with interceptor" {
        let m = mock("HEAD", "/").create();

        let client = HttpClient::builder()
            .interceptor(isahc::interceptor!(request, cx, {
                *request.method_mut() = http::Method::HEAD;
                cx.send(request).await
            }))
            .build()
            .unwrap();

        client.get(server_url()).unwrap();

        m.assert();
    }
}
