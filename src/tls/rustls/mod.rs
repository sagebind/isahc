use super::{Certificate, RootCertStore};

#[cfg(feature = "rustls-tls-native-certs")]
pub(super) mod native_certs;

pub(super) mod platform_verifier;

#[cfg(feature = "rustls-tls-native-certs")]
pub(super) fn load_native_certs() -> Result<RootCertStore, crate::Error> {
    let mut result = rustls_native_certs::load_native_certs();

    if let Some(e) = result.errors.pop() {
        // return Err(create_curl_error(curl_sys::CURLE_SSL_CACERT_BADFILE, e));
    }

    Ok(RootCertStore::custom(
        result
            .certs
            .into_iter()
            .map(|cert| Certificate::from_der(cert)),
    ))
}
