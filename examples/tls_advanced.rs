//! Demonstrates more advanced TLS customization features with certificates
//! created in memory.

use isahc::{
    HttpClient,
    config::Configurable,
    error::ErrorKind,
    tls::{Identity, PrivateKey, TlsConfig, TrustStore},
};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair, PublicKeyData};

fn create_identity() -> Identity {
    // Since this is just an example, we will generate a random self-signed
    // certificate and private key pair to use as our identity to the server. In
    // a real application, you would likely obtain an encrypted key pair issued
    // by the owner of the server, and transmitted in a secure manner.
    //
    // We are using rcgen to do this, which is a popular and convenient library,
    // and also demonstrates Isahc's ability to interop with other TLS
    // libraries.
    let key_pair = KeyPair::generate().unwrap();

    // Split into separate owned types.
    let public_key = key_pair.public_key_pem();
    let private_key = key_pair.der_bytes().to_vec();

    // Define an identity from the key pair. This takes ownership of the pair
    // and does not require any copies. Note how DER and PEM can be mixed and
    // matched as needed.
    Identity::from_pem(public_key, Some(PrivateKey::from_der(private_key, None)))
}

fn create_trust_store() -> TrustStore {
    // Here we will create a custom trust store that trusts only a set of
    // certificates we specify. These will also be randomly generated for
    // demonstration purposes, but

    // First create a key pair we will use to sign our custom certs.
    let key_pair = KeyPair::generate().unwrap();

    // Here's the parameters we'll use to generate new certs.
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

    // Build the custom trust store, generating multiple certs and adding them
    // to the store.
    TrustStore::builder()
        .certificate_from_pem(params.self_signed(&key_pair).unwrap().pem())
        .certificate_from_der(params.self_signed(&key_pair).unwrap().der().clone())
        .build()
}

fn main() -> Result<(), isahc::Error> {
    tracing_subscriber::fmt::init();

    let client = HttpClient::builder()
        // Customize the HTTP client with custom TLS configuration.
        .tls_config(
            TlsConfig::builder()
                .identity(create_identity())
                .trust_store(create_trust_store())
                .build(),
        )
        .build()?;

    // Of course, we expect this to fail because we're requiring the server to
    // use bogus certs, which it won't be.
    let error = client.get("https://example.org").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::BadServerCertificate);

    Ok(())
}
