use isahc::HttpClient;
use mockito::{mock, server_url, Matcher};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "change HTTP method with interceptor" {
        let m = mock("HEAD", "/").create();

        async fn intercept(mut request: http::Request<isahc::Body>, mut cx: isahc::interceptors::Context<'_>) -> Result<isahc::http::Response<isahc::Body>, Box<dyn std::error::Error>> {
            *request.method_mut() = http::Method::HEAD;
            Ok(cx.send(request).await?)
        }

        // let intercept = |mut request: http::Request<isahc::Body>, mut cx: isahc::interceptors::Context<'_>| Box::pin(async {
        //     *request.method_mut() = http::Method::HEAD;
        //     Ok(cx.send(request).await?)
        // });

        let client = HttpClient::builder()
            // .interceptor(move |mut request: http::Request<isahc::Body>, mut cx: isahc::interceptors::Context<'_>| async move {
            //     *request.method_mut() = http::Method::HEAD;
            //     Ok(cx.send(request).await?)
            // })
            .interceptor(intercept)
            .build()
            .unwrap();

        client.get(server_url()).unwrap();

        m.assert();
    }
}
