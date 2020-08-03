use crossbeam_utils::thread;
use isahc::prelude::*;
use mockito::{mock, server_address, server_url};
use std::{
    io::{BufRead, BufReader, Write},
    net::{Shutdown, IpAddr, TcpListener, TcpStream},
};

#[test]
fn no_proxy() {
    let m = mock("GET", "/").create();

    Request::get(server_url())
        .proxy(None)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.assert();
}

#[test]
fn http_proxy() {
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

#[test]
fn socks4_proxy() {
    // Set up a TCP listener that will implement a simple SOCKS4 proxy.
    let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();

    // Create the proxy URI for our listener.
    let proxy_uri = http::Uri::builder()
        .scheme("socks4")
        .authority(proxy_listener.local_addr().unwrap().to_string().as_str())
        .path_and_query("/")
        .build()
        .unwrap();

    let upstream_port = server_address().port();
    let upstream_ip = match server_address().ip() {
        IpAddr::V4(ip) => ip,
        _ => panic!(),
    };

    // Set up our upstream HTTP test server.
    let m = mock("GET", "/").create();

    // Set up a scope to clean up background threads.
    thread::scope(move |s| {
        // Spawn a simple SOCKS4 server to test against.
        s.spawn(move |_| {
            let mut client_writer = proxy_listener.accept().unwrap().0;
            let mut client_reader = BufReader::new(client_writer.try_clone().unwrap());

            // Read connect packet.
            client_reader.fill_buf().unwrap();

            // Header
            assert_eq!(client_reader.buffer()[0], 4);
            assert_eq!(client_reader.buffer()[1], 1);
            client_reader.consume(2);

            // Destination port
            assert_eq!(&client_reader.buffer()[..2], upstream_port.to_be_bytes());
            client_reader.consume(2);

            // Destination address
            assert_eq!(&client_reader.buffer()[..4], upstream_ip.octets());
            client_reader.consume(4);

            // User ID
            loop {
                let byte = client_reader.buffer()[0];
                client_reader.consume(1);
                if byte == 0 {
                    break;
                }
            }

            // Connect to upstream.
            let upstream = TcpStream::connect(server_address()).unwrap();

            // Send response packet.
            client_writer.write_all(&[0, 0x5a, 0, 0, 0, 0, 0, 0]).unwrap();
            client_writer.flush().unwrap();

            // Copy bytes in and to the upstream in parallel.
            thread::scope(|s| {
                s.spawn(|_| {
                    std::io::copy(&mut client_reader, &mut &upstream).unwrap();
                });

                std::io::copy(&mut &upstream, &mut client_writer).unwrap();
                client_writer.shutdown(Shutdown::Both).unwrap();
            }).unwrap();
        });

        // Send a request...
        Request::get(server_url())
            .proxy(proxy_uri)
            .body(())
            .unwrap()
            .send()
            .unwrap();

        // ...expecting to receive it through the proxy.
        m.assert();
    }).unwrap();
}

#[test]
fn proxy_blacklist_works() {
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
