//! Configuration options related to SSL/TLS.

use super::SetOpt;
use curl::easy::{Easy2, SslOpt};
use std::{
    iter::FromIterator,
    ops::{BitOr, BitOrAssign},
    path::PathBuf,
};

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

    /// Get a certificate and private key from a PKCS #12-encoded file.
    ///
    /// Use [`pkcs12_file`][ClientCertificate::pkcs12_file] instead.
    #[inline]
    #[doc(hidden)]
    #[deprecated(
        since = "1.4.0",
        note = "please use the more clearly-named `pkcs12_file` instead"
    )]
    pub fn p12_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self::pkcs12_file(path, password)
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

/// A public CA certificate bundle file.
#[derive(Clone, Debug)]
pub struct CaCertificate {
    /// Path to the certificate bundle file. Currently only file paths are
    /// supported.
    path: PathBuf,
}

impl CaCertificate {
    /// Get a CA certificate from a path to a certificate bundle file.
    ///
    /// The certificate file is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn file(ca_bundle_path: impl Into<PathBuf>) -> Self {
        Self {
            path: ca_bundle_path.into(),
        }
    }
}

impl SetOpt for CaCertificate {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.cainfo(&self.path)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Ciphers(String);

impl FromIterator<String> for Ciphers {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Ciphers(iter.into_iter().collect::<Vec<_>>().join(":"))
    }
}

impl SetOpt for Ciphers {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.ssl_cipher_list(&self.0)
    }
}

/// A flag that can be used to alter the behavior of SSL/TLS connections.
///
/// Most options are for disabling security checks that introduce security
/// risks, but may be required as a last resort.
#[derive(Clone, Copy, Debug)]
pub struct SslOption(usize);

impl Default for SslOption {
    fn default() -> Self {
        Self::NONE
    }
}

impl SslOption {
    /// An empty set of options. This is the default.
    pub const NONE: Self = SslOption(0);

    /// Disables certificate validation.
    ///
    /// # Warning
    ///
    /// You should think very carefully before using this method. If invalid
    /// certificates are trusted, *any* certificate for any site will be trusted
    /// for use. This includes expired certificates. This introduces significant
    /// vulnerabilities, and should only be used as a last resort.
    pub const DANGER_ACCEPT_INVALID_CERTS: Self = SslOption(0b0001);

    /// Disables hostname verification on certificates.
    ///
    /// # Warning
    ///
    /// You should think very carefully before you use this method. If hostname
    /// verification is not used, any valid certificate for any site will be
    /// trusted for use from any other. This introduces a significant
    /// vulnerability to man-in-the-middle attacks.
    pub const DANGER_ACCEPT_INVALID_HOSTS: Self = SslOption(0b0010);

    /// Disables certificate revocation checks for backends where such behavior
    /// is present.
    ///
    /// This option is only supported for Schannel (the native Windows SSL
    /// library).
    pub const DANGER_ACCEPT_REVOKED_CERTS: Self = SslOption(0b0100);

    const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for SslOption {
    type Output = Self;

    fn bitor(mut self, other: Self) -> Self {
        self |= other;
        self
    }
}

impl BitOrAssign for SslOption {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl SetOpt for SslOption {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        let mut opt = SslOpt::new();
        opt.no_revoke(self.contains(Self::DANGER_ACCEPT_REVOKED_CERTS));

        easy.ssl_options(&opt)?;
        easy.ssl_verify_peer(!self.contains(Self::DANGER_ACCEPT_INVALID_CERTS))?;
        easy.ssl_verify_host(!self.contains(Self::DANGER_ACCEPT_INVALID_HOSTS))
    }
}

#[cfg(test)]
mod tests {
    use super::SslOption;

    #[test]
    fn default_ssl_options() {
        let options = SslOption::default();

        assert!(!options.contains(SslOption::DANGER_ACCEPT_INVALID_CERTS));
        assert!(!options.contains(SslOption::DANGER_ACCEPT_INVALID_HOSTS));
        assert!(!options.contains(SslOption::DANGER_ACCEPT_REVOKED_CERTS));
    }

    #[test]
    fn ssl_option_invalid_certs() {
        let options = SslOption::DANGER_ACCEPT_INVALID_CERTS;

        assert!(options.contains(SslOption::DANGER_ACCEPT_INVALID_CERTS));
        assert!(!options.contains(SslOption::DANGER_ACCEPT_INVALID_HOSTS));

        let options = SslOption::DANGER_ACCEPT_INVALID_HOSTS;

        assert!(!options.contains(SslOption::DANGER_ACCEPT_INVALID_CERTS));
        assert!(options.contains(SslOption::DANGER_ACCEPT_INVALID_HOSTS));
    }
}
