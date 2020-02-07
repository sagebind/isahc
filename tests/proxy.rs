use crossbeam_utils::thread;
use isahc::prelude::*;
use mockito::{mock, server_url};
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "http proxy" {
        // URI of our test server, which we will treat as a proxy.
        let proxy = server_url().parse::<http::Uri>().unwrap();
        // Fake upstream URI to connect to.
        let upstream = "http://127.0.0.2:1234/".parse::<http::Uri>().unwrap();

        // We should receive the request instead, following the HTTP proxy
        // protocol. The request-target should be the absolute URI of our
        // upstream request target (see [RFC
        // 7230](https://tools.ietf.org/html/rfc7230), sections 5.3 and 5.7).
        let m = mock("GET", upstream.to_string().as_str())
            // Host should be the upstream authority, not the proxy host.
            .match_header("host", upstream.authority().unwrap().as_str())
            .match_header("proxy-connection", "Keep-Alive")
            .create();

        Request::get(upstream)
            .proxy(proxy)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }

    // test "socks4 proxy" {
    //     // Set up a TCP listener that will implement a simple SOCKS4 proxy.
    //     let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();

    //     // Create the proxy URI for our listener.
    //     let proxy_uri = http::Uri::builder()
    //         .scheme("socks4")
    //         .authority(proxy_listener.local_addr().unwrap().to_string().as_str())
    //         .path_and_query("/")
    //         .build()
    //         .unwrap();

    //     // Ensure our background test helper threads are cleaned up.
    //     thread::scope(move |s| {
    //         s.spawn(move |s| {
    //             for stream in proxy_listener.incoming() {
    //                 let mut stream = BufReader::new(stream.unwrap());

    //                 s.spawn(move |s| {
    //                     // Read connect packet.
    //                     let buf = stream.fill_buf().unwrap();

    //                     assert_eq!(buf[0], 4);
    //                     assert_eq!(buf[1], 1);
    //                 });
    //             }
    //         });

    //         // Set up our upstream HTTP test server.
    //         let m = mock("GET", "/").create();

    //         // Send a request...
    //         Request::get(server_url())
    //         .proxy(proxy_uri)
    //         .body(())
    //         .unwrap()
    //         .send()
    //         .unwrap();

    //         // ...expecting to receive it through the proxy.
    //         m.assert();
    //     }).unwrap();
    // }

    test "proxy blacklist works" {
        // This time, the proxy is the fake one.
        let proxy = "http://127.0.0.2:1234/".parse::<http::Uri>().unwrap();
        // Our test server is upstream (we don't expect the proxy to be used).
        let upstream = server_url().parse::<http::Uri>().unwrap();

        let m = mock("GET", "/").create();

        Request::get(&upstream)
            .proxy(proxy)
            // Exclude our upstream from the proxy we set.
            .proxy_blacklist(Some(upstream.host().unwrap().to_string()))
            .body(())
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }
}
