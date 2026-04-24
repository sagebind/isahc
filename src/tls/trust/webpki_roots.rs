use crate::tls::TrustStore;
use std::sync::LazyLock;

impl TrustStore {
    /// Validate server certificates against the collection of trusted root
    /// certificates published by Mozilla, via the
    /// [`webpki-root-certs`](https://crates.io/crates/webpki-root-certs) crate.
    ///
    /// This method is available when the `webpki-roots` feature is enabled, and
    /// also becomes the default trust store if not otherwise specified when the
    /// `rustls-tls-webpki-roots` feature is enabled.
    ///
    /// # Security considerations
    ///
    /// The collection of root certificates is distributed with the library and
    /// will be compiled into the binary. This is extremely portable, as this
    /// trust store will behave identically across many platforms and
    /// environments. However, it also means that the root certificates cannot
    /// be updated without recompiling your application, and binaries you have
    /// already distributed will not receive any updates to the list that
    /// introduce new certificates or revoke existing ones. This can pose a
    /// security risk depending on how your application is used.
    ///
    /// If your application is distributed in something like an immutable
    /// container image where system root certificates will not receive updates
    /// anyway, then this may be a good option. If instead you are providing an
    /// application that will be run by users on a conventional operating
    /// system, then consider using the system's native root certificate store
    /// via [`TrustStore::native`] instead, which will receive updates as part
    /// of the operating system's regular update process.
    pub fn webpki_roots() -> Self {
        // The list can't change, so generate our representation of it only once
        // and reuse it.
        static WEBPKI_ROOTS: LazyLock<TrustStore> = LazyLock::new(|| {
            webpki_root_certs::TLS_SERVER_ROOT_CERTS
                .iter()
                .fold(TrustStore::builder(), |builder, cert| {
                    builder.certificate_from_der(cert)
                })
                .build()
        });

        WEBPKI_ROOTS.clone()
    }
}
