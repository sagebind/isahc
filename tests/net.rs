use isahc::{config::IpVersion, error::ErrorKind, prelude::*, Request};
use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, Ipv6Addr, Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    thread,
};
use testserver::mock;

#[macro_use]
mod utils;

// Check if the test host supports IPv6.
fn is_ipv6_supported() -> bool {
    if let Ok(addrs) = "localhost:0".to_socket_addrs() {
        for addr in addrs {
            if addr.is_ipv6() {
                return true;
            }
        }
    }

    false
}

#[test]
fn local_addr_returns_expected_address() {
    let m = mock!();

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(m.requests_received(), 1);
    assert_eq!(response.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    assert!(response.local_addr().unwrap().port() > 0);
}

#[test]
fn remote_addr_returns_expected_address() {
    let m = mock!();

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(m.requests_received(), 1);
    assert_eq!(response.remote_addr(), Some(m.addr()));
}

#[test]
fn local_and_remote_addr_returns_expected_addresses_on_error() {
    let server = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = server.local_addr().unwrap();

    thread::spawn(move || {
        let (mut client, _) = server.accept().unwrap();
        client.write_all(b"foobar").unwrap();
        client.flush().unwrap();
    });

    let error = isahc::get(format!("http://localhost:{}", addr.port())).unwrap_err();

    assert_eq!(error.remote_addr(), Some(addr));
    assert_eq!(error.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    assert!(error.local_addr().unwrap().port() > 0);
}

#[test]
fn ipv4_only_will_not_connect_to_ipv6() {
    if !is_ipv6_supported() {
        eprintln!("skipping test because host does not support IPv6");
        return;
    }

    // Create server on IPv6 only.
    let server = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).unwrap();
    let port = server.local_addr().unwrap().port();

    let result = Request::get(format!("http://localhost:{}", port))
        .ip_version(IpVersion::V4)
        .body(())
        .unwrap()
        .send();

    assert_matches!(result, Err(e) if e == ErrorKind::ConnectionFailed);
}

#[test]
fn ipv6_only_will_not_connect_to_ipv4() {
    if !is_ipv6_supported() {
        eprintln!("skipping test because host does not support IPv6");
        return;
    }

    // Create server on IPv4 only.
    let server = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = server.local_addr().unwrap().port();

    let result = Request::get(format!("http://localhost:{}", port))
        .ip_version(IpVersion::V6)
        .body(())
        .unwrap()
        .send();

    assert_matches!(result, Err(e) if e == ErrorKind::ConnectionFailed);
}

#[test]
fn any_ip_version_uses_ipv4_or_ipv6() {
    // Create an IPv4 listener.
    let server_v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = server_v4.local_addr().unwrap().port();

    // Create an IPv6 listener on the same port.
    let server_v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).unwrap();

    fn respond(client: &mut TcpStream, response: &[u8]) -> io::Result<()> {
        let _ = client.read(&mut [0; 8192])?;
        client.write_all(response)?;
        client.flush()?;
        client.shutdown(Shutdown::Both)
    }

    // Each server response with a string indicating which address family used.
    thread::spawn(move || {
        let (mut client, _) = server_v4.accept().unwrap();
        respond(
            &mut client,
            b"HTTP/1.1 200 OK\r\ncontent-length:4\r\n\r\nipv4",
        )
        .unwrap();
    });
    thread::spawn(move || {
        let (mut client, _) = server_v6.accept().unwrap();
        respond(
            &mut client,
            b"HTTP/1.1 200 OK\r\ncontent-length:4\r\n\r\nipv6",
        )
        .unwrap();
    });

    let mut response = Request::get(format!("http://localhost:{}", port))
        .ip_version(IpVersion::Any)
        .body(())
        .unwrap()
        .send()
        .unwrap();

    let text = response.text().unwrap();

    // On dual stack hosts, this should equal "ipv6", but some environments like
    // WSL2 don't support IPv6 properly.
    assert_matches!(text.as_str(), "ipv4" | "ipv6");

    // Our local address should correspond to the same family as the server.
    if text == "ipv6" {
        assert!(response.local_addr().unwrap().is_ipv6());
    } else {
        assert!(response.local_addr().unwrap().is_ipv4());
    }
}
