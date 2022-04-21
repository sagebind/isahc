//! Configuration options related to SSL/TLS.

use crate::{has_tls_engine, TlsEngine};

use super::SetOpt;
use curl::easy::{Easy2, SslOpt, SslVersion};
use once_cell::sync::Lazy;
use std::path::PathBuf;

/// A flag that can be used to alter the behavior of SSL/TLS connections.
///
/// Most options are for disabling security checks that introduce security
/// risks, but may be required as a last resort.
#[derive(Debug, Default)]
#[must_use = "builders have no effect if unused"]
pub struct TlsConfigBuilder {
    /// Custom root CA certificates.
    root_certs: Vec<Certificate>,

    root_cert_path: Option<PathBuf>,
    native_roots: bool,
    issuer_cert: Option<Certificate>,
    issuer_cert_path: Option<PathBuf>,
    ciphers: Vec<String>,
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
    pub fn root_ca_certificate(mut self, cert: Certificate) -> Self {
        self.root_certs.push(cert);
        self
    }

    pub fn root_ca_native(mut self, b: bool) -> Self {
        self.native_roots = b;
        self
    }

    /// Get a CA certificate from a path to a certificate bundle file.
    ///
    /// The certificate file is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn root_ca_certificate_path(mut self, ca_bundle_path: impl Into<PathBuf>) -> Self {
        self.root_cert_path = Some(ca_bundle_path.into());
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
    /// legitimate, but it can _also_ be used by the server to verify that _you_
    /// are legitimate. If the server asks the client to present an approved
    /// certificate before continuing, then this sets the certificate(s) that
    /// will be supplied in response.
    ///
    /// If a format is not supported by the underlying SSL/TLS engine, an error
    /// will be returned when attempting to send a request using the offending
    /// certificate.
    ///
    /// The default value is none.
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
    pub fn client_certificate(mut self, cert: KeyPair) -> Self {
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

    /// Set a list of ciphers to use for SSL/TLS connections.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// engine in use. You can find an up-to-date list of potential cipher names
    /// at <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in using whatever ciphers the
    /// configured TLS backend enables by default.
    pub fn ciphers<I, T>(mut self, ciphers: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.ciphers = ciphers.into_iter().map(T::into).collect();
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
    pub fn build(&self) -> TlsConfig {
        TlsConfig {
            ciphers: if self.ciphers.is_empty() {
                None
            } else {
                Some(self.ciphers.join(":"))
            },
            root_certs_pem: if self.root_certs.is_empty() {
                if self.native_roots && has_tls_engine(TlsEngine::Rustls) {
                    let mut bundle = String::new();

                    for cert in native_roots() {
                        bundle.push_str(&cert.pem);
                    }

                    Some(bundle)
                } else {
                    None
                }
            } else {
                let mut bundle = String::new();

                for cert in &self.root_certs {
                    bundle.push_str(&cert.pem);
                }

                Some(bundle)
            },
            root_cert_path: self.root_cert_path.clone(),
            issuer_cert: self.issuer_cert.clone(),
            min_version: self.min_version.clone(),
            max_version: self.max_version.clone(),
            danger_accept_invalid_certs: self.danger_accept_invalid_certs,
            danger_accept_invalid_hosts: self.danger_accept_invalid_hosts,
            danger_accept_revoked_certs: self.danger_accept_revoked_certs,
        }
    }
}

/// Configuration for making SSL/TLS connections.
#[derive(Clone, Debug)]
pub struct TlsConfig {
    ciphers: Option<String>,

    /// A string containing one or more certificates in PEM format.
    root_certs_pem: Option<String>,

    root_cert_path: Option<PathBuf>,
    issuer_cert: Option<Certificate>,
    min_version: Option<ProtocolVersion>,
    max_version: Option<ProtocolVersion>,
    danger_accept_invalid_certs: bool,
    danger_accept_invalid_hosts: bool,
    danger_accept_revoked_certs: bool,
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

        let mut opt = SslOpt::new();
        opt.no_revoke(self.danger_accept_revoked_certs);

        easy.ssl_options(&opt)?;
        easy.ssl_verify_peer(!self.danger_accept_invalid_certs)?;
        easy.ssl_verify_host(!self.danger_accept_invalid_hosts)?;

        easy.ssl_min_max_version(
            self.min_version
                .as_ref()
                .map(ProtocolVersion::curl_version)
                .unwrap_or(SslVersion::Default),
            self.max_version
                .as_ref()
                .map(ProtocolVersion::curl_version)
                .unwrap_or(SslVersion::Default),
        )?;

        if let Some(pem) = self.root_certs_pem.as_ref() {
            easy.ssl_cainfo_blob(pem.as_bytes())?;
        } else if let Some(path) = self.root_cert_path.as_ref() {
            easy.cainfo(path)?;
        }

        if let Some(cert) = self.issuer_cert.as_ref() {
            easy.issuer_cert_blob(cert.pem.as_bytes())?;
        }

        Ok(())
    }
}

/// Supported TLS versions that can be used.
#[derive(Clone, Debug)]
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
    pub fn from_der<B: AsRef<[u8]>>(certificate: B) -> Self {
        let mut base64_spec = data_encoding::BASE64.specification();
        base64_spec.wrap.width = 64;
        base64_spec.wrap.separator.push_str("\r\n");
        let base64 = base64_spec.encoding().unwrap();

        let mut pem = String::new();

        pem.push_str("-----BEGIN CERTIFICATE-----\r\n");
        base64.encode_append(certificate.as_ref(), &mut pem);
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
    pub fn from_pem<B: AsRef<[u8]>>(cert: B) -> Self {
        Self {
            pem: String::from_utf8(cert.as_ref().to_vec()).unwrap(),
        }
    }
}

pub struct KeyPair {
    /// The actual certificate containing the public key.
    certificate: Certificate,

    /// The private key corresponding to the public key.
    private: Vec<u8>,

    /// Password to decrypt the private key file.
    password: Option<String>,
}

impl KeyPair {
    pub fn from_pkcs12() -> Self {
        todo!()
    }
}

#[derive(Clone, Debug)]
enum PathOrBlob {
    Path(PathBuf),
    Blob(Vec<u8>),
}

/// A client certificate for SSL/TLS client validation.
///
/// Note that this isn't merely an X.509 certificate, but rather a certificate
/// and private key pair.
#[derive(Clone, Debug)]
pub struct ClientCertificate {
    /// Name of the cert format.
    format: &'static str,

    /// The certificate data, either a path or a blob.
    data: PathOrBlob,

    /// Private key corresponding to the SSL/TLS certificate.
    private_key: Option<PrivateKey>,

    /// Password to decrypt the certificate file.
    password: Option<String>,
}

impl ClientCertificate {
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

impl SetOpt for ClientCertificate {
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

/// Get native root certificates trusted by the operating system, if any.
#[allow(unreachable_code)]
fn native_roots() -> Vec<Certificate> {
    static NATIVE: Lazy<Vec<Certificate>> = Lazy::new(|| {
        #[cfg(feature = "rustls-native-certs")]
        {
            match rustls_native_certs::load_native_certs() {
                Ok(certs) => {
                    return certs
                        .into_iter()
                        .map(|cert| Certificate::from_der(cert.0))
                        .collect();
                }
                Err(e) => {
                    log::warn!("failed to load native certificate chain: {}", e);
                }
            }
        }

        Vec::new()
    });

    NATIVE.clone()
}
