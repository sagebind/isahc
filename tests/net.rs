use isahc::prelude::*;
use std::net::Ipv4Addr;
use testserver::endpoint;

#[test]
fn local_addr_returns_expected_address() {
    let endpoint = endpoint!();

    let response = isahc::get(endpoint.url()).unwrap();

    assert_eq!(endpoint.requests().len(), 1);
    assert_eq!(response.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    assert!(response.local_addr().unwrap().port() > 0);
}

#[test]
fn remote_addr_returns_expected_address() {
    let endpoint = endpoint!();

    let response = isahc::get(endpoint.url()).unwrap();

    assert_eq!(endpoint.requests().len(), 1);
    assert_eq!(response.remote_addr(), Some(endpoint.addr()));
}
