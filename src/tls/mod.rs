//! Configuration options related to SSL/TLS for supporting [HTTPS] requests.
//!
//! # Backends
//!
//! Isahc does not implement the TLS protocol itself, but instead uses one of
//! various available *TLS backends* which implement support. With the default
//! crate configuration, Isahc will use the target platform's "native" TLS
//! implementation:
//!
//! - Windows: [Schannel]
//! - macOS and iOS: [Secure Transport]
//! - Linux and other UNIX-like systems: [OpenSSL] or one of its API-compatible
//!   forks are usually considered the "default" TLS engine on most Linux
//!   distributions and are treated as the native backend here.
//!
//! Regardless of platform, support for TLS is gated behind the optional `tls`
//! crate feature, which while enabled by default can be disabled if you don't
//! need or want to make HTTPS requests. Enabling the crate feature enables this
//! API module, but does not select which backend to use. To select which
//! backend to use, additional crate features are provided:
//!
//! - `native-tls`: Use the target platform's native TLS engine, as described
//!   earlier. This is the default.
//! - `rustls-tls`: Use a statically-linked [rustls], a modern TLS library written in Rust.
//! - `rustls-tls-native-certs`: Use [rustls] along with the
//!   [rustls-native-certs] library to allow rustls to use the platform's native
//!   root certificate store.
//!
//! If using rustls without native cert support, your application will need to
//! provide its own certificates to use for verification, as none are included by
//! default.
//!
//! There are pros and cons to different backends, and none are best for all use
//! cases. For a more in-depth look at the available backends see the [wiki
//! article on TLS
//! backends](https://github.com/sagebind/isahc/wiki/TLS-Backends).
//!
//! [HTTPS]: https://en.wikipedia.org/wiki/HTTPS
//! [OpenSSL]: https://www.openssl.org
//! [rustls]: https://github.com/rustls/rustls
//! [rustls-native-certs]: https://github.com/rustls/rustls-native-certs
//! [Schannel]: https://docs.microsoft.com/en-us/windows/win32/com/schannel
//! [Secure Transport]:
//!     https://developer.apple.com/documentation/security/secure_transport

use crate::{
    config::{proxy::SetOptProxy, request::SetOpt},
    error::create_curl_error,
};
use curl::easy::{Easy2, SslOpt, SslVersion};
use std::path::PathBuf;

mod identity;
mod roots;

pub use self::{
    identity::{Identity, PrivateKey},
    roots::RootCertStore,
};

#[cfg(not(any(feature = "native-tls", feature = "rustls-tls")))]
compile_error!("`tls` feature is enabled, but no TLS backend was selected.");

// #[cfg(all(feature = "native-tls", feature = "rustls-tls"))]
// compile_error!("multiple TLS engines cannot be enabled at the same time");

/// A builder for creating a custom SSL/TLS connector configuration.
#[derive(Debug, Default)]
#[must_use = "builders have no effect if unused"]
pub struct TlsConfigBuilder {
    root_cert_store: RootCertStore,
    issuer_cert: Option<Certificate>,
    issuer_cert_path: Option<PathBuf>,
    identity: Option<Identity>,
    ciphers: Option<String>,
    min_version: Option<ProtocolVersion>,
    max_version: Option<ProtocolVersion>,
    danger_accept_invalid_certs: bool,
    danger_accept_invalid_hosts: bool,
    danger_accept_revoked_certs: bool,
}

impl TlsConfigBuilder {
    /// Set the certificate store containing trusted root certificates to use
    /// for validating server certificates.
    ///
    /// Root certificates are used for validating the authenticity of a server
    /// before proceeding with a request. If the server presents a certificate
    /// that matches the server's information, and is signed by a certificate
    /// authority either in the root certificate store or is itself trusted by
    /// another certificate in the store, then the server is considered to be
    /// legitimate.
    ///
    /// The default setting varies on how Isahc is compiled:
    ///
    /// - When using the native TLS API the default is
    ///   [`RootCertStore::native`].
    /// - When using rustls, the default is an empty store, unless the
    ///   `rustls-tls-native-certs` crate feature is enabled, in which case the
    ///   default is [`RootCertStore::native`].
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::tls::{Certificate, RootCertStore, TlsConfig};
    ///
    /// let config = TlsConfig::builder()
    ///     // Use the native certificate store
    ///     .root_cert_store(RootCertStore::native())
    ///     // Use a specific certificate bundle file
    ///     .root_cert_store(RootCertStore::from_file("/etc/certs/cabundle.pem"))
    ///     // Use custom certs in memory
    ///     .root_cert_store(RootCertStore::custom([
    ///         Certificate::from_pem("(some long PEM string)"),
    ///     ]))
    ///     // You could even include a certificate bundle in your binary
    ///     .root_cert_store(RootCertStore::custom([
    ///         Certificate::from_pem(include_str!("../../tests/certs/isrgrootx1.pem")),
    ///     ]))
    ///     .build();
    /// ```
    pub fn root_cert_store(mut self, store: RootCertStore) -> Self {
        self.root_cert_store = store;
        self
    }

    /// Specify a specific CA certificate which server certificates must be
    /// signed by.
    ///
    /// Typically, server certificates are validated by checking whether they
    /// are signed by a trusted root certificate, or by any intermediate CA
    /// certificate authorized by a root certificate. If this is not strict
    /// enough for your use case, you can additionally validate that server
    /// certificates must be signed by a **specific** CA certificate. This
    /// method specifies that certificate which servers must match exactly.
    ///
    /// By default, no issuer certificate is set.
    pub fn issuer_certificate(mut self, cert: Certificate) -> Self {
        self.issuer_cert = Some(cert);
        self
    }

    /// Add a custom client certificate to use for client authentication, also
    /// known as *mutual TLS*.
    ///
    /// SSL/TLS is often used by the client to verify that the server is
    /// legitimate (one-way), but with *mutual TLS* (mTLS)  it is _also_ used by
    /// the server to verify that _you_ are legitimate (two-way). If the server
    /// asks the client to present an approved certificate before continuing,
    /// then this sets the certificate chain that will be used to prove
    /// authenticity.
    ///
    /// If a certificate or key format given is not supported by the underlying
    /// SSL/TLS engine, an error will be returned when attempting to send a
    /// request using the offending certificate or key.
    ///
    /// By default, no client certificate is set.
    ///
    /// # Backend support
    ///
    /// Support for mutual TLS varies between the available TLS backends. Here
    /// are some current limitations of note:
    ///
    /// - Schannel and Secure Transport require certificates and private keys to
    ///   be presented together inside a PKCS #12 archive. This can be an actual
    ///   archive or one in memory.
    /// - Mutual TLS with Rustls is not supported at all.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::tls::{Identity, PrivateKey, TlsConfig};
    ///
    /// let config = TlsConfig::builder()
    ///     .identity(Identity::from_pem_file(
    ///         "client.pem",
    ///         PrivateKey::pem_file("key.pem", String::from("secret"))
    ///     ))
    ///     .build();
    /// # Ok::<(), isahc::Error>(())
    /// ```
    pub fn identity(mut self, identity: Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Set the minimum allowed protocol version for secure connections.
    ///
    /// If specified, the client will attempt to negotiate secure connections
    /// using the given protocol version or newer if possible. If a server only
    /// supports protocols older than the set minimum then the connection will
    /// be terminated with a
    /// [`ConnectionFailed`][crate::error::ErrorKind::ConnectionFailed] error.
    ///
    /// The default value if not set depends on the TLS backend Isahc is
    /// configured to use. With most backends on most modern systems the default
    /// will be TLS v1.2, but if you want to be certain then you may want to set
    /// a minimum version explicitly.
    pub fn min_version(mut self, version: ProtocolVersion) -> Self {
        self.min_version = Some(version);
        self
    }

    /// Set a maximum allowed protocol version for secure connections.
    ///
    /// If specified, the client will attempt to negotiate secure connections
    /// using the given protocol version or older if possible. If a server only
    /// supports protocols newer than the set maximum then the connection will
    /// be terminated with a
    /// [`ConnectionFailed`][crate::error::ErrorKind::ConnectionFailed] error.
    ///
    /// By default no maximum version is enforced, however the newest version
    /// that can be used will be dependent on what newest protocols are
    /// supported by the underlying TLS engine.
    pub fn max_version(mut self, version: ProtocolVersion) -> Self {
        self.max_version = Some(version);
        self
    }

    /// Set a list of ciphers or cipher suites to allow.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// backend in use. You can find an up-to-date list of potential cipher names
    /// at <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in using whatever ciphers the
    /// configured TLS backend chooses to enable by default.
    ///
    /// # Warning
    ///
    /// Not all cipher suites are created equal, as many have been marked as
    /// being insecure over time. This method allows you to enable potentially
    /// insecure cipher suites that are not enabled by default and may introduce
    /// security vulnerabilities into your application.
    pub fn ciphers<I, T>(mut self, ciphers: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        let mut iter = ciphers.into_iter();

        // If an empty list is provided, reset to default. Otherwise build up a
        // string in curl format containing the cipher names.
        if let Some(first) = iter.next() {
            let mut ciphers = first.as_ref().to_owned();

            for cipher in iter {
                ciphers.push(':');
                ciphers.push_str(cipher.as_ref());
            }

            self.ciphers = Some(ciphers);
        } else {
            self.ciphers = None;
        }

        self
    }

    /// Disables all server certificate validation.
    ///
    /// By default this is enabled.
    ///
    /// # Warning
    ///
    /// **You should think very carefully before using this method.** If invalid
    /// certificates are trusted, *any* certificate for any site will be trusted
    /// for use. This includes expired certificates. This introduces significant
    /// vulnerabilities, and should only be used as a last resort.
    pub fn danger_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.danger_accept_invalid_certs = accept;
        self
    }

    /// Disables hostname verification on server certificates.
    ///
    /// By default this is enabled.
    ///
    /// # Warning
    ///
    /// **You should think very carefully before you use this method.** If
    /// hostname verification is not used, any valid certificate for any site
    /// will be trusted for use from any other. This introduces a significant
    /// vulnerability to man-in-the-middle attacks.
    pub fn danger_accept_invalid_hosts(mut self, accept: bool) -> Self {
        self.danger_accept_invalid_hosts = accept;
        self
    }

    /// Disables certificate revocation checks for backends where such behavior
    /// is present.
    ///
    /// By default this is enabled.
    ///
    /// This option is only supported when using the Schannel TLS backend (the
    /// native Windows SSL/TLS implementation). On other backends this option
    /// will likely have no effect regardless of setting.
    ///
    /// # Warning
    ///
    /// **You should think very carefully before you use this method.** Backends
    /// implementing revocation checks do so to ensure that a certificate that
    /// appears valid has not been reported as compromised. Disabling this can
    /// increase your application's vulnerability to malicious servers.
    pub fn danger_accept_revoked_certs(mut self, accept: bool) -> Self {
        self.danger_accept_revoked_certs = accept;
        self
    }

    /// Create a new [`TlsConfig`] based on this builder.
    pub fn build(self) -> TlsConfig {
        TlsConfig {
            ciphers: self.ciphers,
            root_cert_store: self.root_cert_store,
            issuer_cert: self.issuer_cert,
            issuer_cert_path: self.issuer_cert_path,
            identity: self.identity,
            min_version: self.min_version.as_ref().map(ProtocolVersion::curl_version),
            max_version: self.max_version.as_ref().map(ProtocolVersion::curl_version),
            danger_accept_invalid_certs: self.danger_accept_invalid_certs,
            danger_accept_invalid_hosts: self.danger_accept_invalid_hosts,
            curl_flags: {
                let mut options = SslOpt::new();
                options.no_revoke(self.danger_accept_revoked_certs);
                options
            },
        }
    }
}

/// Configuration for making SSL/TLS connections.
#[derive(Clone, Debug)]
pub struct TlsConfig {
    root_cert_store: RootCertStore,
    issuer_cert: Option<Certificate>,
    issuer_cert_path: Option<PathBuf>,
    identity: Option<Identity>,

    /// List of ciphers to use, in a string format compatible with curl.
    ciphers: Option<String>,
    min_version: Option<SslVersion>,
    max_version: Option<SslVersion>,
    danger_accept_invalid_certs: bool,
    danger_accept_invalid_hosts: bool,
    curl_flags: SslOpt,
}

impl TlsConfig {
    /// Create an instance of the default SSL/TLS configuration.
    ///
    /// The default configuration varies depending on the runtime environment
    /// and how Isahc is compiled. By default, Isahc uses the platform's
    /// "native" TLS implementation, and will use TLS ciphers and versions that
    /// are enabled by default on the system. Naturally, this will vary from
    /// machine to machine, so less-secure methods may be used if the user's
    /// system is configured that way.
    ///
    /// If Isahc is built to use an alternative TLS implementation such as
    /// rustls, then that implementation will be used on all platforms, and the
    /// default settings will come from the defaults provided by that bundled
    /// TLS library version.
    ///
    /// This is equivalent to the [`Default::default`] method.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a new [`TlsConfigBuilder`] for creating a custom SSL/TLS
    /// configuration. The initial configuration of the builder will start from
    /// the default TLS settings as described in [`TlsConfigBuilder::new`].
    #[inline]
    pub fn builder() -> TlsConfigBuilder {
        TlsConfigBuilder::default()
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl SetOpt for TlsConfig {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        if let Some(ciphers) = self.ciphers.as_ref() {
            easy.ssl_cipher_list(ciphers)?;
        }

        easy.ssl_options(&self.curl_flags)?;
        easy.ssl_verify_peer(!self.danger_accept_invalid_certs)?;
        easy.ssl_verify_host(!self.danger_accept_invalid_hosts)?;

        // Rustls only supports TLS 1.2+. Currently the backend in curl just
        // ignores this option entirely, so give a more helpful message if the
        // user tries to explicitly use something older.
        if has_tls_engine(TlsEngine::Rustls)
            && !matches!(
                self.max_version,
                Some(SslVersion::Tlsv12)
                    | Some(SslVersion::Tlsv13)
                    | Some(SslVersion::Default)
                    | None
            )
        {
            return Err(create_curl_error(
                curl_sys::CURLE_SSL_ENGINE_INITFAILED,
                "rustls only supports TLS 1.2+",
            ));
        }

        easy.ssl_min_max_version(
            self.min_version.unwrap_or(SslVersion::Default),
            self.max_version.unwrap_or(SslVersion::Default),
        )?;

        self.root_cert_store.set_opt(easy)?;

        if let Some(cert) = self.issuer_cert.as_ref() {
            easy.issuer_cert_blob(cert.pem.as_bytes())?;
        }

        if let Some(path) = self.issuer_cert_path.as_ref() {
            easy.issuer_cert(path)?;
        }

        if let Some(identity) = self.identity.as_ref() {
            identity.set_opt(easy)?;
        }

        Ok(())
    }
}

impl SetOptProxy for TlsConfig {
    fn set_opt_proxy<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        if let Some(ciphers) = self.ciphers.as_ref() {
            easy.proxy_ssl_cipher_list(ciphers)?;
        }

        easy.proxy_ssl_options(&self.curl_flags)?;
        easy.proxy_ssl_verify_peer(self.danger_accept_invalid_certs)?;
        easy.proxy_ssl_verify_host(self.danger_accept_invalid_hosts)?;

        easy.proxy_ssl_min_max_version(
            self.min_version.unwrap_or(SslVersion::Default),
            self.max_version.unwrap_or(SslVersion::Default),
        )?;

        self.root_cert_store.set_opt_proxy(easy)?;

        if let Some(cert) = self.issuer_cert.as_ref() {
            easy.proxy_issuer_cert_blob(cert.pem.as_bytes())?;
        }

        if let Some(path) = self.issuer_cert_path.as_ref() {
            easy.proxy_issuer_cert(path)?;
        }

        if let Some(identity) = self.identity.as_ref() {
            identity.set_opt_proxy(easy)?;
        }

        Ok(())
    }
}

/// Possible SSL/TLS versions that can be used.
///
/// Not all versions are supported by all TLS backends, and can vary depending
/// how a user's system is configured. Requesting or requiring a version that
/// the TLS backend does not allow or support will result in a runtime error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProtocolVersion {
    /// SSL v2. Support for this version is disabled in the default build
    /// configuration and not even available any more in most TLS backends due
    /// to being considered insecure.
    Sslv2,

    /// SSL v3. Support for this version is disabled in the default build
    /// configuration and not even available any more in most TLS backends due
    /// to being considered insecure.
    Sslv3,

    /// TLS v1.0. Considered insecure.
    Tlsv10,

    /// TLS v1.1. Considered insecure.
    Tlsv11,

    /// TLS v1.2
    Tlsv12,

    /// TLS v1.3
    Tlsv13,
}

impl ProtocolVersion {
    const fn curl_version(&self) -> SslVersion {
        match self {
            Self::Sslv2 => SslVersion::Sslv2,
            Self::Sslv3 => SslVersion::Sslv3,
            Self::Tlsv10 => SslVersion::Tlsv10,
            Self::Tlsv11 => SslVersion::Tlsv11,
            Self::Tlsv12 => SslVersion::Tlsv12,
            Self::Tlsv13 => SslVersion::Tlsv13,
        }
    }
}

/// An X.509 digital certificate.
#[derive(Clone, Debug)]
pub struct Certificate {
    /// Curl prefers to work in the PEM format, so internally we do as well.
    pem: String,
}

impl Certificate {
    /// Use one or more DER-encoded certificates stored in memory.
    ///
    /// The certificates are not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    #[cfg(feature = "data-encoding")]
    pub fn from_der<B: AsRef<[u8]>>(der: B) -> Self {
        let mut base64_spec = data_encoding::BASE64.specification();
        base64_spec.wrap.width = 64;
        base64_spec.wrap.separator.push_str("\r\n");
        let base64 = base64_spec.encoding().unwrap();

        let mut pem = String::new();

        pem.push_str("-----BEGIN CERTIFICATE-----\r\n");
        base64.encode_append(der.as_ref(), &mut pem);
        pem.push_str("-----END CERTIFICATE-----\r\n");

        Self::from_pem(pem)
    }

    /// Use one or more PEM-encoded certificates in the given byte buffer.
    ///
    /// The certificate object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The certificates are not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn from_pem<B: AsRef<[u8]>>(pem: B) -> Self {
        Self {
            pem: String::from_utf8(pem.as_ref().to_vec()).unwrap(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
enum TlsEngine {
    Rustls,
    Schannel,
    SecureTransport,
}

fn has_tls_engine(engine: TlsEngine) -> bool {
    if let Some(version) = crate::curl_info().ssl_version() {
        match engine {
            TlsEngine::Rustls => version.contains("rustls/"),
            TlsEngine::Schannel => version.contains("Schannel"),
            TlsEngine::SecureTransport => version.contains("SecureTransport"),
        }
    } else {
        false
    }
}
