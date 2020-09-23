use isahc::prelude::*;
use mockito::{mock, server_address, server_url};
use std::net::Ipv4Addr;

#[test]
fn local_addr_returns_expected_address() {
    let m = mock("GET", "/").create();

    let response = isahc::get(server_url()).unwrap();

    m.assert();

    assert_eq!(response.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    assert!(response.local_addr().unwrap().port() > 0);
}

#[test]
fn remote_addr_returns_expected_address() {
    let m = mock("GET", "/").create();

    let response = isahc::get(server_url()).unwrap();

    m.assert();

    assert_eq!(response.remote_addr(), Some(server_address()));
}
