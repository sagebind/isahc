//! Configuration options related to SSL/TLS.

use crate::{has_tls_engine, TlsEngine};

use super::SetOpt;
use curl::easy::{Easy2, SslOpt, SslVersion};
use std::path::PathBuf;

/// A flag that can be used to alter the behavior of SSL/TLS connections.
///
/// Most options are for disabling security checks that introduce security
/// risks, but may be required as a last resort.
#[derive(Debug, Default)]
#[must_use = "builders have no effect if unused"]
pub struct TlsConfigBuilder {
    root_certs: Vec<Certificate>,
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
    /// Add a certificate to the trusted roots. This is used to verify the
    /// authenticity of the server.
    ///
    /// This takes precedence over
    /// [`TlsConfigBuilder::root_ca_certificate_path`].
    ///
    /// The default value is none.
    ///
    /// # Notes
    ///
    /// On Windows it may be necessary to combine this with
    /// [`TlsConfigBuilder::danger_accept_revoked_certs`] in order to work
    /// depending on the contents of your CA bundle.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::{config::CaCertificate, prelude::*, HttpClient};
    ///
    /// let client = HttpClient::builder()
    ///     .ssl_ca_certificate(CaCertificate::file("ca.pem"))
    ///     .build()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    pub fn root_ca_certificate(self, cert: Certificate) -> Self {
        self.root_cert_store(RootCertStore::custom([cert]))
    }

    /// Get a CA certificate from a path to a certificate bundle file.
    ///
    /// The certificate file is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn root_ca_certificate_path(self, ca_bundle_path: impl Into<PathBuf>) -> Self {
        self.root_cert_store(RootCertStore::file(ca_bundle_path))
    }

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
    /// use isahc::config::tls::{Certificate, RootCertStore, TlsConfig};
    ///
    /// let config = TlsConfig::builder()
    ///     // Use the native certificate store
    ///     .root_cert_store(RootCertStore::native())
    ///     // Use a specific certificate bundle file
    ///     .root_cert_store(RootCertStore::file("/etc/certs/cabundle.pem"))
    ///     // Use custom certs in memory
    ///     .root_cert_store(RootCertStore::custom([
    ///         Certificate::from_pem("(some long PEM string)"),
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

    /// Add a custom client certificate to use for client authentication.
    ///
    /// SSL/TLS is often used by the client to verify that the server is
    /// legitimate (one-way), but it can _also_ be used by the server to verify
    /// that _you_ are legitimate (two-way). If the server asks the client to
    /// present an approved certificate before continuing, then this sets the
    /// certificate chain that will be used to prove authenticity.
    ///
    /// If a certificate or key format given is not supported by the underlying
    /// SSL/TLS engine, an error will be returned when attempting to send a
    /// request using the offending certificate or key.
    ///
    /// By default, no client certificate is set.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::{
    ///     config::{ClientCertificate, PrivateKey},
    ///     prelude::*,
    ///     Request,
    /// };
    ///
    /// let response = Request::get("localhost:3999")
    ///     .ssl_client_certificate(ClientCertificate::pem_file(
    ///         "client.pem",
    ///         PrivateKey::pem_file("key.pem", String::from("secret")),
    ///     ))
    ///     .body(())?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    ///
    /// ```
    /// use isahc::{
    ///     config::{ClientCertificate, PrivateKey},
    ///     prelude::*,
    ///     HttpClient,
    /// };
    ///
    /// let client = HttpClient::builder()
    ///     .ssl_client_certificate(ClientCertificate::pem_file(
    ///         "client.pem",
    ///         PrivateKey::pem_file("key.pem", String::from("secret")),
    ///     ))
    ///     .build()?;
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

    /// Disables certificate validation.
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

    /// Disables hostname verification on certificates.
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
    /// This option is only supported when using the Schannel TLS backend (the
    /// native Windows SSL/TLS implementation).
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
            min_version: self.min_version.as_ref().map(ProtocolVersion::curl_version),
            max_version: self.max_version.as_ref().map(ProtocolVersion::curl_version),
            danger_accept_invalid_certs: self.danger_accept_invalid_certs,
            danger_accept_invalid_hosts: self.danger_accept_invalid_hosts,
            ssl_options: {
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
    /// List of ciphers to use, in a string format compatible with curl.
    ciphers: Option<String>,

    root_cert_store: RootCertStore,

    issuer_cert: Option<Certificate>,
    min_version: Option<SslVersion>,
    max_version: Option<SslVersion>,
    danger_accept_invalid_certs: bool,
    danger_accept_invalid_hosts: bool,

    /// SSL flags to pass to curl.
    ssl_options: SslOpt,
}

impl TlsConfig {
    /// Create an instance of the default SSL/TLS configuration.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a new [`TlsConfigBuilder`] for creating a custom SSL/TLS
    /// configuration.
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

        easy.ssl_options(&self.ssl_options)?;
        easy.ssl_verify_peer(!self.danger_accept_invalid_certs)?;
        easy.ssl_verify_host(!self.danger_accept_invalid_hosts)?;

        easy.ssl_min_max_version(
            self.min_version.unwrap_or(SslVersion::Default),
            self.max_version.unwrap_or(SslVersion::Default),
        )?;

        self.root_cert_store.set_opt(easy)?;

        if let Some(cert) = self.issuer_cert.as_ref() {
            easy.issuer_cert_blob(cert.pem.as_bytes())?;
        }

        Ok(())
    }
}

/// A store that provides a collection of trusted root certificates.
#[derive(Clone, Debug)]
pub struct RootCertStore(RootCertStoreImpl);

#[derive(Clone, Debug)]
enum RootCertStoreImpl {
    Empty,
    Native,
    Dir(PathBuf),
    File(PathBuf),

    /// A custom bundle of certificates stored in PEM format.
    Bundle(String),
}

impl RootCertStore {
    /// Create an empty certificate store.
    pub const fn empty() -> Self {
        Self(RootCertStoreImpl::Empty)
    }

    /// Use the platform's native certificate store, if any.
    ///
    /// On Windows, macOS, and iOS this involves using the certificate
    /// management features provided by the operating system. On Linux and other
    /// UNIX-like systems this typically will use a shared certificate bundle
    /// managed by the distribution or system administrator. In most cases, this
    /// will also respect environment variables that override where to look for
    /// trusted certificates.
    ///
    /// This is typically the default certificate store used for most
    /// applications.
    pub fn native() -> Self {
        Self(RootCertStoreImpl::Native)
    }

    /// Use a given directory containing multiple certificates as a certificate
    /// store.
    ///
    /// This option only works properly when using OpenSSL or its derivatives,
    /// GnuTLS, or mbedTLS.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::config::tls::RootCertStore;
    ///
    /// let store = RootCertStore::dir("/etc/cert-dir");
    /// ```
    pub fn dir<P: Into<PathBuf>>(path: P) -> Self {
        Self(RootCertStoreImpl::Dir(path.into()))
    }

    /// Use a file containing a bundle of certificates in PEM format.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::config::tls::RootCertStore;
    ///
    /// let store = RootCertStore::file("/etc/certs/cabundle.pem");
    /// ```
    pub fn file<P: Into<PathBuf>>(path: P) -> Self {
        Self(RootCertStoreImpl::File(path.into()))
    }

    /// Create a custom certificate store containing exactly the given
    /// certificates.
    ///
    /// Server certificates will be verified using only certificates given.
    ///
    /// This type of store is supported by most TLS backends, including OpenSSL,
    /// rustls, and Secure Transport.
    pub fn custom<I>(certificates: I) -> Self
    where
        I: IntoIterator<Item = Certificate>,
    {
        // Generate a PEM bundle.
        let mut bundle = String::new();

        for cert in certificates {
            bundle.push_str(&cert.pem);
        }

        Self(RootCertStoreImpl::Bundle(bundle))
    }
}

impl Default for RootCertStore {
    fn default() -> Self {
        if has_tls_engine(TlsEngine::Rustls) && !cfg!(feature = "rustls-native-certs") {
            Self::empty()
        } else {
            Self::native()
        }
    }
}

impl SetOpt for RootCertStore {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match &self.0 {
            RootCertStoreImpl::Empty => Ok(()),
            RootCertStoreImpl::Dir(path) => easy.capath(path),
            RootCertStoreImpl::File(path) => easy.cainfo(path),
            RootCertStoreImpl::Bundle(bundle) => easy.ssl_cainfo_blob(bundle.as_bytes()),
            RootCertStoreImpl::Native => {
                // When using rustls, we have to load native certs ourselves,
                // and a couple possible ways are available to us.
                if has_tls_engine(TlsEngine::Rustls) {
                    #[cfg(feature = "rustls-native-certs")]
                    {
                        static NATIVE_CERTS: once_cell::sync::OnceCell<RootCertStore> =
                            once_cell::sync::OnceCell::new();

                        fn load_native_certs() -> std::io::Result<RootCertStore> {
                            Ok(RootCertStore::custom(
                                rustls_native_certs::load_native_certs()?
                                    .into_iter()
                                    .map(|cert| Certificate::from_der(cert.0)),
                            ))
                        }

                        match NATIVE_CERTS.get_or_try_init(load_native_certs) {
                            Ok(certs) => return certs.set_opt(easy),
                            Err(e) => {
                                let mut error = curl::Error::new(77);
                                error.set_extra(e.to_string());
                                return Err(error);
                            },
                        }
                    }

                    Err(curl::Error::new(77))
                } else {
                    // When using true native, the TLS backend will handle certs
                    // automatically.
                    Ok(())
                }
            }
        }
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
            _ => SslVersion::Default,
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

#[derive(Clone, Debug)]
enum PathOrBlob {
    Path(PathBuf),
    Blob(Vec<u8>),
}

/// Holds a X.509 certificate, potentially other certificates in its chain of
/// trust, along with a corresponding private key.
#[derive(Clone, Debug)]
pub struct Identity {
    /// Name of the cert format.
    format: &'static str,

    /// The certificate data, either a path or a blob.
    data: PathOrBlob,

    /// Private key corresponding to the SSL/TLS certificate.
    private_key: Option<PrivateKey>,

    /// Password to decrypt the certificate file.
    password: Option<String>,
}

impl Identity {
    /// Use a PEM-encoded certificate stored in the given byte buffer.
    ///
    /// The certificate object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The certificate is not parsed or validated here. If the certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn pem<B, P>(bytes: B, private_key: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<PrivateKey>>,
    {
        Self {
            format: "PEM",
            data: PathOrBlob::Blob(bytes.into()),
            private_key: private_key.into(),
            password: None,
        }
    }

    /// Use a DER-encoded certificate stored in the given byte buffer.
    ///
    /// The certificate object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The certificate is not parsed or validated here. If the certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn der<B, P>(bytes: B, private_key: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<PrivateKey>>,
    {
        Self {
            format: "DER",
            data: PathOrBlob::Blob(bytes.into()),
            private_key: private_key.into(),
            password: None,
        }
    }

    /// Use a certificate and private key from a PKCS #12 archive stored in the
    /// given byte buffer.
    ///
    /// The certificate object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The certificate is not parsed or validated here. If the certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn pkcs12<B, P>(bytes: B, password: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<String>>,
    {
        Self {
            format: "P12",
            data: PathOrBlob::Blob(bytes.into()),
            private_key: None,
            password: password.into(),
        }
    }

    /// Get a certificate from a PEM-encoded file.
    ///
    /// The certificate file is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn pem_file(path: impl Into<PathBuf>, private_key: impl Into<Option<PrivateKey>>) -> Self {
        Self {
            format: "PEM",
            data: PathOrBlob::Path(path.into()),
            private_key: private_key.into(),
            password: None,
        }
    }

    /// Get a certificate from a DER-encoded file.
    ///
    /// The certificate file is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn der_file(path: impl Into<PathBuf>, private_key: impl Into<Option<PrivateKey>>) -> Self {
        Self {
            format: "DER",
            data: PathOrBlob::Path(path.into()),
            private_key: private_key.into(),
            password: None,
        }
    }

    /// Get a certificate and private key from a PKCS #12-encoded file.
    ///
    /// The certificate file is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn pkcs12_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self {
            format: "P12",
            data: PathOrBlob::Path(path.into()),
            private_key: None,
            password: password.into(),
        }
    }
}

impl SetOpt for Identity {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.ssl_cert_type(self.format)?;

        match &self.data {
            PathOrBlob::Path(path) => easy.ssl_cert(path.as_path()),
            PathOrBlob::Blob(bytes) => easy.ssl_cert_blob(bytes.as_slice()),
        }?;

        if let Some(key) = self.private_key.as_ref() {
            key.set_opt(easy)?;
        }

        if let Some(password) = self.password.as_ref() {
            easy.key_password(password)?;
        }

        Ok(())
    }
}

/// A private key file.
#[derive(Clone, Debug)]
pub struct PrivateKey {
    /// Key format name.
    format: &'static str,

    /// The certificate data, either a path or a blob.
    data: PathOrBlob,

    /// Password to decrypt the key file.
    password: Option<String>,
}

impl PrivateKey {
    /// Use a PEM-encoded private key stored in the given byte buffer.
    ///
    /// The private key object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The key is not parsed or validated here. If the key is malformed or the
    /// format is not supported by the underlying SSL/TLS engine, an error will
    /// be returned when attempting to send a request using the offending key.
    pub fn pem<B, P>(bytes: B, password: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<String>>,
    {
        Self {
            format: "PEM",
            data: PathOrBlob::Blob(bytes.into()),
            password: password.into(),
        }
    }

    /// Use a DER-encoded private key stored in the given byte buffer.
    ///
    /// The private key object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The key is not parsed or validated here. If the key is malformed or the
    /// format is not supported by the underlying SSL/TLS engine, an error will
    /// be returned when attempting to send a request using the offending key.
    pub fn der<B, P>(bytes: B, password: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<String>>,
    {
        Self {
            format: "DER",
            data: PathOrBlob::Blob(bytes.into()),
            password: password.into(),
        }
    }

    /// Get a PEM-encoded private key file.
    ///
    /// The key file is not loaded or validated here. If the file does not exist
    /// or the format is not supported by the underlying SSL/TLS engine, an
    /// error will be returned when attempting to send a request using the
    /// offending key.
    pub fn pem_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self {
            format: "PEM",
            data: PathOrBlob::Path(path.into()),
            password: password.into(),
        }
    }

    /// Get a DER-encoded private key file.
    ///
    /// The key file is not loaded or validated here. If the file does not exist
    /// or the format is not supported by the underlying SSL/TLS engine, an
    /// error will be returned when attempting to send a request using the
    /// offending key.
    pub fn der_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self {
            format: "DER",
            data: PathOrBlob::Path(path.into()),
            password: password.into(),
        }
    }
}

impl SetOpt for PrivateKey {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.ssl_key_type(self.format)?;

        match &self.data {
            PathOrBlob::Path(path) => easy.ssl_key(path.as_path()),
            PathOrBlob::Blob(bytes) => easy.ssl_key_blob(bytes.as_slice()),
        }?;

        if let Some(password) = self.password.as_ref() {
            easy.key_password(password)?;
        }

        Ok(())
    }
}
