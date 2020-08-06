use isahc::prelude::*;
use mockito::{mock, server_address, server_url};
use std::net::Ipv4Addr;

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    #[ignore = "getsockname not enabled yet upstream"]
    test "client_addr returns expected address" {
        let m = mock("GET", "/").create();

        let response = isahc::get(server_url()).unwrap();

        m.assert();

        assert_eq!(response.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
        assert!(response.local_addr().unwrap().port() > 0);
    }

    test "server_addr returns expected address" {
        let m = mock("GET", "/").create();

        let response = isahc::get(server_url()).unwrap();

        m.assert();

        assert_eq!(response.remote_addr(), Some(server_address()));
    }
}
