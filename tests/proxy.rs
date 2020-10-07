use isahc::prelude::*;
use testserver::{mock, socks4::Socks4Server};

#[test]
fn no_proxy() {
    let m = mock!();

    Request::get(m.url())
        .proxy(None)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.requests().len(), 1);
}

#[test]
fn http_proxy() {
    // URI of our test server, which we will treat as a proxy.
    let m = mock!();
    let proxy = m.url().parse::<http::Uri>().unwrap();

    // Fake upstream URI to connect to.
    let upstream = "http://127.0.0.2:1234/".parse::<http::Uri>().unwrap();

    Request::get(upstream.clone())
        .proxy(proxy)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    // We should receive the request instead, following the HTTP proxy
    // protocol. The request-target should be the absolute URI of our
    // upstream request target (see [RFC
    // 7230](https://tools.ietf.org/html/rfc7230), sections 5.3 and 5.7).
    assert_eq!(m.request().url, upstream.to_string());
    // Host should be the upstream authority, not the proxy host.
    m.request().expect_header("host", upstream.authority().unwrap().as_str());
    m.request().expect_header("proxy-connection", "Keep-Alive");
}

#[test]
#[cfg_attr(tarpaulin, ignore)]
fn socks4_proxy() {
    // Set up a simple SOCKS4 proxy.
    let proxy_server = Socks4Server::new("127.0.0.1:0").unwrap();

    // Create the proxy URI for our listener.
    let proxy_uri = http::Uri::builder()
        .scheme("socks4")
        .authority(proxy_server.addr().to_string().as_str())
        .path_and_query("/")
        .build()
        .unwrap();

    // Run the proxy server in the background.
    proxy_server.spawn();

    // Set up our upstream HTTP test server.
    let m = mock!();

    // Send a request...
    Request::get(m.url())
        .header("connection", "close")
        .proxy(proxy_uri)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    // ...expecting to receive it through the proxy.
    assert_eq!(m.requests().len(), 1);
}

#[test]
fn proxy_blacklist_works() {
    // This time, the proxy is the fake one.
    let proxy = "http://127.0.0.2:1234/".parse::<http::Uri>().unwrap();

    // Our test server is upstream (we don't expect the proxy to be used).
    let m = mock!();
    let upstream = m.url().parse::<http::Uri>().unwrap();

    Request::get(&upstream)
        .proxy(proxy)
        // Exclude our upstream from the proxy we set.
        .proxy_blacklist(Some(upstream.host().unwrap().to_string()))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(m.requests().len(), 1);
}
