use isahc::{
    config::{Configurable, VersionNegotiation},
    http::Version,
    Request,
    RequestExt,
};
use std::io::Write;
use testserver::mock;

#[test]
fn http11_by_default() {
    let m = mock!();

    let response = isahc::get(m.url()).unwrap();

    assert_eq!(response.version(), Version::HTTP_11);
}

#[test]
fn http09_not_allowed_by_default() {
    let m = mock! {
        _ => writer |writer| {
            write!(writer, "<html>hello</html>\r\n").unwrap();
        },
    };

    let err = isahc::get(m.url()).unwrap_err();

    assert_eq!(err, isahc::error::ErrorKind::ProtocolViolation);
}

#[test]
fn enable_http09() {
    let m = mock! {
        _ => writer |writer| {
            write!(writer, "<html>hello</html>\r\n").unwrap();
        },
    };

    let response = Request::get(m.url())
        .version_negotiation(VersionNegotiation::default().http09(true))
        .body(())
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.version(), Version::HTTP_09);
}
