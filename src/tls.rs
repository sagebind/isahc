//! TLS client configuration.

use std::{fs::File, io::Write};

use curl::easy::Easy2;

#[allow(unsafe_code)]
pub(crate) fn init_default_tls_config<H>(easy: &mut Easy2<H>) {
    // Load native root certificates and add them to the handle.
    #[cfg(feature = "rustls-native-certs")]
    {
        if let Ok(certs) = rustls_native_certs::load_native_certs() {
            // TODO: The rustls backend in curl doesn't support
            // `CURLOPT_CAINFO_BLOB` yet. For now use an awful hack of
            // generating a PEM file on-demand for curl to use.
            let mut tmp = File::create(".tmp-roots.pem").unwrap();
            for cert in certs {
                let pem = pem::Pem {
                    tag: String::from("CERTIFICATE"),
                    contents: cert.0,
                };
                tmp.write_all(pem::encode(&pem).as_bytes()).unwrap();
            }
            tmp.flush().unwrap();
            drop(tmp);
            easy.cainfo(".tmp-roots.pem").unwrap();
        }
    }
}
