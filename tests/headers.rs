use futures_lite::future::block_on;
use isahc::{prelude::*, HttpClient, Request};
use std::{
    io::{self, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread,
    time::Duration,
};
use testserver::mock;

#[test]
fn accept_headers_populated_by_default() {
    let m = mock!();

    isahc::get(m.url()).unwrap();

    m.request().expect_header("accept", "*/*");
    m.request()
        .expect_header("accept-encoding", "deflate, gzip");
}

#[test]
fn user_agent_contains_expected_format() {
    let m = mock!();

    isahc::get(m.url()).unwrap();

    m.request()
        .expect_header_regex("user-agent", r"^curl/\S+ isahc/\S+$");
}

// Issue [#209](https://github.com/sagebind/isahc/issues/209)
#[test]
fn setting_an_empty_header_sends_a_header_with_no_value() {
    let m = mock!();

    Request::get(m.url())
        .header("an-empty-header", "")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.request().expect_header("an-empty-header", "");
}

// Issue [#209](https://github.com/sagebind/isahc/issues/209)
#[test]
fn setting_a_blank_header_sends_a_header_with_no_value() {
    let m = mock!();

    Request::get(m.url())
        .header("an-empty-header", "    ")
        .body(())
        .unwrap()
        .send()
        .unwrap();

    m.request().expect_header("an-empty-header", "");
}

// Issue [#190](https://github.com/sagebind/isahc/issues/190)
#[test]
fn override_client_default_user_agent() {
    let m = mock!();

    let client = HttpClient::builder()
        .default_header("user-agent", "foo")
        .build()
        .unwrap();

    client.get(m.url()).unwrap();

    m.request().expect_header("user-agent", "foo");
}

// Issue [#205](https://github.com/sagebind/isahc/issues/205)
#[test]
fn set_title_case_headers_to_true() {
    let m = mock!();

    let client = HttpClient::builder()
        .default_header("foo-BAR", "baz")
        .title_case_headers(true)
        .build()
        .unwrap();

    client.get(m.url()).unwrap();

    assert_eq!(m.request().method(), "GET");
    m.request().expect_header("Foo-Bar", "baz");
}

#[test]
fn header_can_be_inserted_in_httpclient_builder() {
    let m = mock!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(m.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    m.request().expect_header("accept", "*/*");
    m.request()
        .expect_header("accept-encoding", "deflate, gzip");
    m.request().expect_header("X-header", "some-value1");
}

#[test]
fn headers_in_request_builder_must_override_headers_in_httpclient_builder() {
    let m = mock!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .header("X-header", "some-value2")
        .uri(m.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    m.request().expect_header("accept", "*/*");
    m.request()
        .expect_header("accept-encoding", "deflate, gzip");
    m.request().expect_header("X-header", "some-value2");
}

#[test]
fn multiple_headers_with_same_key_can_be_inserted_in_httpclient_builder() {
    let m = mock!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .default_header("X-header", "some-value2")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(m.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    m.request().expect_header("accept", "*/*");
    m.request()
        .expect_header("accept-encoding", "deflate, gzip");
    // Both values should be present.
    m.request().expect_header("X-header", "some-value1");
    m.request().expect_header("X-header", "some-value2");
}

#[test]
fn headers_in_request_builder_must_override_multiple_headers_in_httpclient_builder() {
    let m = mock!();

    let client = HttpClient::builder()
        .default_header("X-header", "some-value1")
        .default_header("X-header", "some-value2")
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .header("X-header", "some-value3")
        .uri(m.url())
        .body(())
        .unwrap();

    let _ = client.send(request).unwrap();

    m.request().expect_header("accept", "*/*");
    m.request()
        .expect_header("accept-encoding", "deflate, gzip");
    m.request().expect_header("X-header", "some-value3");
}

#[test]
fn trailer_headers() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());

    thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;

        consume_request_in_background(&stream);

        stream
            .write_all(
                b"\
            HTTP/1.1 200 OK\r\n\
            transfer-encoding: chunked\r\n\
            trailer: foo\r\n\
            \r\n\
            2\r\n\
            OK\r\n\
            0\r\n\
            foo: bar\r\n\
            \r\n\
        ",
            )
            .unwrap();

        let _ = stream.shutdown(Shutdown::Write);
    });

    let mut body = None;
    let response = isahc::get(url).unwrap().map(|b| {
        body = Some(b);
    });

    thread::spawn(move || {
        io::copy(body.as_mut().unwrap(), &mut io::sink()).unwrap();
    });

    assert_eq!(response.trailer().wait().get("foo").unwrap(), "bar");
}

#[test]
fn trailer_headers_async() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());

    thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;

        consume_request_in_background(&stream);

        stream
            .write_all(
                b"\
            HTTP/1.1 200 OK\r\n\
            transfer-encoding: chunked\r\n\
            trailer: foo\r\n\
            \r\n\
            2\r\n\
            OK\r\n\
            0\r\n\
            foo: bar\r\n\
            \r\n\
            ",
            )
            .unwrap();

        let _ = stream.shutdown(Shutdown::Write);
    });

    block_on(async move {
        let mut body = None;
        let response = isahc::get_async(url).await.unwrap().map(|b| {
            body = Some(b);
        });

        thread::spawn(move || {
            block_on(async move {
                futures_lite::io::copy(body.as_mut().unwrap(), &mut futures_lite::io::sink())
                    .await
                    .unwrap();
            })
        });

        assert_eq!(
            response.trailer().wait_async().await.get("foo").unwrap(),
            "bar"
        );
    });
}

#[test]
fn trailer_headers_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());

    thread::spawn(move || {
        let mut stream = listener.accept().unwrap().0;
        stream.set_nodelay(true).unwrap();

        consume_request_in_background(&stream);

        stream
            .write_all(
                b"\
            HTTP/1.1 200 OK\r\n\
            transfer-encoding: chunked\r\n\
            trailer: foo\r\n\
            \r\n",
            )
            .unwrap();

        for _ in 0..1000 {
            stream.write_all(b"5\r\nhello\r\n").unwrap();
        }

        stream.write_all(b"0\r\n").unwrap();

        thread::sleep(Duration::from_millis(200));

        stream.write_all(b"foo: bar\r\n\r\n").unwrap();

        let _ = stream.shutdown(Shutdown::Write);
    });

    let response = isahc::get(url).unwrap();

    // Since we don't consume the response body and the trailer is in a separate
    // packet from the header, we won't receive the trailer in time.
    assert!(
        response
            .trailer()
            .wait_timeout(Duration::from_millis(10))
            .is_none()
    );
}

fn consume_request_in_background(stream: &TcpStream) {
    let mut stream = stream.try_clone().unwrap();

    thread::spawn(move || {
        let _ = io::copy(&mut stream, &mut io::sink());
        let _ = stream.shutdown(Shutdown::Read);
    });
}
