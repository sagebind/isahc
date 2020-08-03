use isahc::prelude::*;
use mockito::{mock, server_url};
use std::{io, thread, time::Duration};

#[test]
fn metrics_are_disabled_by_default() {
    let m = mock("GET", "/").create();

    let response = isahc::get(server_url()).unwrap();

    assert!(response.metrics().is_none());

    m.assert();
}

#[test]
fn enabling_metrics_causes_metrics_to_be_collected() {
    let m = mock("POST", "/")
        .with_body_from_fn(|body| {
            thread::sleep(Duration::from_millis(10));
            body.write_all(b"hello world")?;
            Ok(())
        })
        .create();

    let client = isahc::HttpClient::builder()
        .metrics(true)
        .build()
        .unwrap();

    let mut response = client.send(Request::post(server_url())
        .body("hello server")
        .unwrap())
        .unwrap();

    let metrics = response.metrics().unwrap().clone();

    assert_eq!(metrics.upload_progress(), (12, 12));

    io::copy(response.body_mut(), &mut io::sink()).unwrap();

    assert_eq!(metrics.download_progress().0, 11);
    assert!(metrics.total_time() > Duration::default());

    m.assert();
}
