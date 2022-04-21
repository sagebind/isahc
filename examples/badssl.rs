//! This example contains a number of manual tests against badssl.com
//! demonstrating several dangerous SSL/TLS options.

use isahc::{config::TlsConfig, error::ErrorKind, prelude::*, Request};

fn main() {
    println!("ssl: {:?}", curl::Version::get().ssl_version());

    // accept expired cert
    Request::get("https://expired.badssl.com")
        .tls_config(TlsConfig::builder()
            .danger_accept_invalid_certs(true)
            .build())
        .body(())
        .unwrap()
        .send()
        .expect("cert should have been accepted");

    // accepting invalid certs alone does not allow invalid hosts
    let error = Request::get("https://wrong.host.badssl.com")
        .tls_config(TlsConfig::builder()
            .danger_accept_invalid_certs(true)
            .build())
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");
    assert_eq!(error, ErrorKind::BadServerCertificate);

    // accept cert with wrong host
    Request::get("https://wrong.host.badssl.com")
        .tls_config(TlsConfig::builder()
            .danger_accept_invalid_hosts(true)
            .build())
        .body(())
        .unwrap()
        .send()
        .expect("cert should have been accepted");

    // accepting certs with wrong host alone does not allow invalid certs
    Request::get("https://expired.badssl.com")
        .tls_config(TlsConfig::builder()
            .danger_accept_invalid_hosts(true)
            .build())
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");
}
