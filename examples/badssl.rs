//! This example contains a number of manual tests against badssl.com
//! demonstrating several dangerous SSL/TLS options.

use isahc::config::SslOption;
use isahc::prelude::*;

fn main() {
    // accept expired cert
    Request::get("https://expired.badssl.com")
        .ssl_options(SslOption::DANGER_ACCEPT_INVALID_CERTS)
        .body(())
        .unwrap()
        .send()
        .expect("cert should have been accepted");

    // accepting invalid certs alone does not allow invalid hosts
    Request::get("https://wrong.host.badssl.com")
        .ssl_options(SslOption::DANGER_ACCEPT_INVALID_CERTS)
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");

    // accept cert with wrong host
    Request::get("https://wrong.host.badssl.com")
        .ssl_options(SslOption::DANGER_ACCEPT_INVALID_HOSTS)
        .body(())
        .unwrap()
        .send()
        .expect("cert should have been accepted");

    // accepting certs with wrong host alone does not allow invalid certs
    Request::get("https://expired.badssl.com")
        .ssl_options(SslOption::DANGER_ACCEPT_INVALID_HOSTS)
        .body(())
        .unwrap()
        .send()
        .expect_err("cert should have been rejected");
}
