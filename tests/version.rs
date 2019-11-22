use isahc::config::VersionNegotiation;
use isahc::prelude::*;
use mockito::{mock, server_url};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "latest compatible negotiation politely asks for HTTP/2" {
        let m = mock("GET", "/")
            .match_header("upgrade", "h2c")
            .create();

        Request::get(server_url())
            .version_negotiation(VersionNegotiation::latest_compatible())
            .body(())
            .unwrap()
            .send()
            .unwrap();

        m.assert();
    }
}
