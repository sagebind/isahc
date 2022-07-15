//! These tests exercise various SSL/TLS options while making requests to [badssl.com](https://badssl.com).

use isahc::{
    error::ErrorKind,
    prelude::*,
    tls::{ProtocolVersion, TlsConfig},
    Request,
};

#[test]
#[cfg_attr(not(feature = "online-tests"), ignore)]
fn accept_expired_cert() {
    Request::get("https://expired.badssl.com")
        .tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_certs(true)
                .build(),
        )
        .body(())
        .unwrap()
        .send()
        .expect("cert should have been accepted");
}

#[test]
#[cfg_attr(not(feature = "online-tests"), ignore)]
fn accepting_invalid_certs_alone_does_not_allow_invalid_hosts() {
    let error = Request::get("https://wrong.host.badssl.com")
        .tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_certs(true)
                .build(),
        )
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");

    assert_eq!(error, ErrorKind::BadServerCertificate);
}

#[test]
#[cfg_attr(not(feature = "online-tests"), ignore)]
fn accept_cert_with_wrong_host() {
    Request::get("https://wrong.host.badssl.com")
        .tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_hosts(true)
                .build(),
        )
        .body(())
        .unwrap()
        .send()
        .expect("cert should have been accepted");
}

#[test]
#[cfg_attr(not(feature = "online-tests"), ignore)]
fn accepting_certs_with_wrong_host_alone_does_not_allow_invalid_certs() {
    Request::get("https://expired.badssl.com")
        .tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_hosts(true)
                .build(),
        )
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");
}

#[test]
#[cfg_attr(not(feature = "online-tests"), ignore)]
fn tls_less_than_min_version_is_rejected() {
    let error = Request::get("https://tls-v1-0.badssl.com:1010")
        .tls_config(
            TlsConfig::builder()
                .min_version(ProtocolVersion::Tlsv12)
                .build(),
        )
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");

    assert_eq!(error, ErrorKind::ConnectionFailed);
}
