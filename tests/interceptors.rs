#![cfg(feature = "unstable-interceptors")]

use isahc::HttpClient;
use testserver::mock;

#[test]
fn change_http_method_with_interceptor() {
    let m = mock!();

    let client = HttpClient::builder()
        .interceptor(isahc::interceptor!(request, cx, {
            *request.method_mut() = http::Method::HEAD;
            cx.send(request).await
        }))
        .build()
        .unwrap();

    client.get(m.url()).unwrap();

    assert_eq!(m.request().method, "HEAD");
}
