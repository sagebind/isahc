use isahc::prelude::*;
use std::{io, thread, time::Duration};
use testserver::endpoint;

#[test]
fn metrics_are_disabled_by_default() {
    let endpoint = endpoint!();

    let response = isahc::get(endpoint.url()).unwrap();

    assert!(response.metrics().is_none());
    assert_eq!(endpoint.requests().len(), 1);
}

#[test]
fn enabling_metrics_causes_metrics_to_be_collected() {
    let endpoint = endpoint! {
        body: |writer| {
            thread::sleep(Duration::from_millis(10));
            writer.write_all(b"hello world")
        },
    };

    let client = isahc::HttpClient::builder()
        .metrics(true)
        .build()
        .unwrap();

    let mut response = client.send(Request::post(endpoint.url())
        .body("hello server")
        .unwrap())
        .unwrap();

    let metrics = response.metrics().unwrap().clone();

    assert_eq!(metrics.upload_progress(), (12, 12));

    io::copy(response.body_mut(), &mut io::sink()).unwrap();

    assert_eq!(metrics.download_progress().0, 11);
    assert!(metrics.total_time() > Duration::default());

    assert_eq!(endpoint.requests().len(), 1);
}
