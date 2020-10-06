use isahc::prelude::*;
use std::{io, time::Duration};
use testserver::mock;

#[test]
fn metrics_are_disabled_by_default() {
    let m = mock!();

    let response = isahc::get(m.url()).unwrap();

    assert!(!m.requests().is_empty());
    assert!(response.metrics().is_none());
}

#[test]
fn enabling_metrics_causes_metrics_to_be_collected() {
    let m = mock! {
        delay: 10ms,
        body: "hello world",
    };

    let client = isahc::HttpClient::builder()
        .metrics(true)
        .build()
        .unwrap();

    let mut response = client.send(Request::post(m.url())
        .body("hello server")
        .unwrap())
        .unwrap();

    let metrics = response.metrics().unwrap().clone();

    assert_eq!(metrics.upload_progress(), (12, 12));

    io::copy(response.body_mut(), &mut io::sink()).unwrap();

    assert_eq!(metrics.download_progress().0, 11);
    assert!(metrics.total_time() > Duration::default());
}
