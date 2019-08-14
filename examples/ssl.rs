//! This example demonstrates HTTPS support, and also demonstrates various
//! errors that are produced when encountering insecure connections.

macro_rules! assert_matches {
    ($e:expr, $expected:pat) => {
        match $e {
            $expected => (),
            actual => panic!("expected '{}' to match pattern '{}', instead got '{:?}'", stringify!($e), stringify!($expected), actual),
        }
    };
}

fn main() -> Result<(), isahc::Error> {
    env_logger::init();

    assert_matches!(isahc::get("https://expired.badssl.com"), Err(isahc::Error::BadServerCertificate(_)));
    assert_matches!(isahc::get("https://wrong.host.badssl.com"), Err(isahc::Error::SSLConnectFailed(_)));
    assert_matches!(isahc::get("https://self-signed.badssl.com"), Err(isahc::Error::SSLConnectFailed(_)));
    assert_matches!(isahc::get("https://untrusted-root.badssl.com"), Err(isahc::Error::SSLConnectFailed(_)));
    assert_matches!(isahc::get("https://revoked.badssl.com"), Err(isahc::Error::SSLConnectFailed(_)));
    assert_matches!(isahc::get("https://pinning-test.badssl.com"), Err(isahc::Error::SSLConnectFailed(_)));

    Ok(())
}
