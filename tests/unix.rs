#![cfg(unix)]

use isahc::{
    prelude::*,
    config::Dial,
};
use std::{
    io::Write,
    os::unix::net::UnixListener,
    thread,
};
use tempfile::TempDir;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "send request to unix socket" {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(include_bytes!("unix_socket_response.http")).unwrap();
        });

        let mut response = Request::get("http://localhost")
            .dial(Dial::unix_socket(socket_path))
            .body(())
            .unwrap()
            .send()
            .unwrap();

        assert_eq!(response.text().unwrap(), "success\n");
    }
}
