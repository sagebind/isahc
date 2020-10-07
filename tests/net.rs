use isahc::prelude::*;
use std::net::Ipv4Addr;
use testserver::mock;

#[test]
fn local_addr_returns_expected_address() {
    let m = mock!();

    let response = isahc::get(m.url()).unwrap();

    assert!(!m.requests().is_empty());
    assert_eq!(response.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    assert!(response.local_addr().unwrap().port() > 0);
}

#[test]
fn remote_addr_returns_expected_address_expected_address() {
    let m = mock!();

    let response = isahc::get(m.url()).unwrap();

    assert!(!m.requests().is_empty());
    assert_eq!(response.remote_addr(), Some(m.addr()));
}
