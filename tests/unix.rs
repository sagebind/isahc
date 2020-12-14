#![cfg(unix)]

use isahc::{
    prelude::*,
    config::Dialer,
};
use std::{
    io::{self, Write},
    os::unix::net::UnixListener,
    thread,
};
use tempfile::TempDir;

#[test]
fn send_request_to_unix_socket() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = stream.try_clone().unwrap();

        thread::spawn(move || {
            io::copy(&mut reader, &mut io::sink()).unwrap();
        });

        stream.write_all(b"\
            HTTP/1.1 200 OK\r\n\
            Content-Length: 8\r\n\
            \r\n\
            success\n\
        ").unwrap();
    });

    let mut response = Request::get("http://localhost")
        .dial(Dialer::unix_socket(socket_path))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.text().unwrap(), "success\n");
}
