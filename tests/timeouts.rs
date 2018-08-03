extern crate chttp;
extern crate env_logger;
extern crate rouille;

use std::env;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::*;
use std::thread;

/// Issue #3
#[test]
fn request_errors_if_read_timeout_is_reached() {
    setup();

    // Spawn a slow server.
    spawn_server(|_| {
        thread::sleep(Duration::from_secs(3));
        rouille::Response::text("hello world")
    });

    // Create an impatient client.
    let mut options = chttp::Options::default();
    options.timeout = Some(Duration::from_secs(2));
    let client = chttp::Client::builder().options(options).build();

    // Send a request.
    let result = client.post("http://localhost:18080", "hello world");

    // Client should time-out.
    assert!(match result {
        Err(chttp::Error::Timeout) => true,
        _ => false,
    });
}

fn setup() {
    env::set_var("RUST_LOG", "chttp=trace,curl=trace");
    env_logger::init();
}

fn spawn_server(handler: fn(&rouille::Request) -> rouille::Response) -> Arc<AtomicBool> {
    let cancellation_token = Arc::new(AtomicBool::default());
    let listen_token = cancellation_token.clone();
    let server = rouille::Server::new("localhost:18080", handler).unwrap();

    thread::spawn(move || {
        while !listen_token.load(Ordering::SeqCst) {
            server.poll();
        }
    });

    cancellation_token
}
