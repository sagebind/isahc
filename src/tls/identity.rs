use crate::{
    config::setopt::{EasyHandle, SetOpt, SetOptError, SetOptProxy},
    handler::BlobOptions,
};
use curl_sys::{
    CURLOPT_PROXY_SSLCERT_BLOB, CURLOPT_PROXY_SSLKEY_BLOB, CURLOPT_SSLCERT_BLOB,
    CURLOPT_SSLKEY_BLOB,
};
use std::{path::PathBuf, sync::Arc};

/// A cryptographic identity used to authenticate the client with a server.
///
/// Holds a X.509 certificate along with potentially other certificates in its
/// chain of trust and a corresponding private key. This collection of
/// certificates is used to authenticate the client to the server if the server
/// requests client authentication during the SSL/TLS handshake. This process is
/// also known as *mutual TLS* (mTLS).
#[derive(Clone, Debug)]
pub struct Identity {
    /// The format of the client certificate.
    format: CertFormat,

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
    pub fn from_pem<B, P>(bytes: B, private_key: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<PrivateKey>>,
    {
        Self {
            format: CertFormat::Pem,
            data: PathOrBlob::Blob(Arc::from(bytes.into())),
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
    pub fn from_der<B, P>(bytes: B, private_key: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<PrivateKey>>,
    {
        Self {
            format: CertFormat::Der,
            data: PathOrBlob::Blob(Arc::from(bytes.into())),
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
    pub fn from_pkcs12<B, P>(bytes: B, password: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<String>>,
    {
        Self {
            format: CertFormat::Pkcs12,
            data: PathOrBlob::Blob(Arc::from(bytes.into())),
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
    pub fn from_pem_file(
        path: impl Into<PathBuf>,
        private_key: impl Into<Option<PrivateKey>>,
    ) -> Self {
        Self {
            format: CertFormat::Pem,
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
    pub fn from_der_file(
        path: impl Into<PathBuf>,
        private_key: impl Into<Option<PrivateKey>>,
    ) -> Self {
        Self {
            format: CertFormat::Der,
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
    pub fn from_pkcs12_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self {
            format: CertFormat::Pkcs12,
            data: PathOrBlob::Path(path.into()),
            private_key: None,
            password: password.into(),
        }
    }
}

impl SetOpt for Identity {
    fn set_opt(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        easy.ssl_cert_type(self.format.as_str())?;

        match &self.data {
            PathOrBlob::Path(path) => easy.ssl_cert(path.as_path()),
            PathOrBlob::Blob(bytes) => unsafe {
                easy.setopt_blob_nocopy(CURLOPT_SSLCERT_BLOB, bytes)
            },
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

impl SetOptProxy for Identity {
    fn set_opt_proxy(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        easy.proxy_sslcert_type(self.format.as_str())?;

        match &self.data {
            PathOrBlob::Path(path) => easy.proxy_sslcert(path.to_str().unwrap()),
            PathOrBlob::Blob(bytes) => unsafe {
                easy.setopt_blob_nocopy(CURLOPT_PROXY_SSLCERT_BLOB, bytes)
            },
        }?;

        if let Some(key) = self.private_key.as_ref() {
            key.set_opt_proxy(easy)?;
        }

        if let Some(password) = self.password.as_ref() {
            easy.proxy_key_password(password)?;
        }

        Ok(())
    }
}

/// A private key file.
#[derive(Clone, Debug)]
pub struct PrivateKey {
    /// The format of the private key.
    format: CertFormat,

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
    pub fn from_pem<B, P>(bytes: B, password: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<String>>,
    {
        Self {
            format: CertFormat::Pem,
            data: PathOrBlob::Blob(Arc::from(bytes.into())),
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
    pub fn from_der<B, P>(bytes: B, password: P) -> Self
    where
        B: Into<Vec<u8>>,
        P: Into<Option<String>>,
    {
        Self {
            format: CertFormat::Der,
            data: PathOrBlob::Blob(Arc::from(bytes.into())),
            password: password.into(),
        }
    }

    /// Get a PEM-encoded private key file.
    ///
    /// The key file is not loaded or validated here. If the file does not exist
    /// or the format is not supported by the underlying SSL/TLS engine, an
    /// error will be returned when attempting to send a request using the
    /// offending key.
    pub fn from_pem_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self {
            format: CertFormat::Pem,
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
    pub fn from_der_file(path: impl Into<PathBuf>, password: impl Into<Option<String>>) -> Self {
        Self {
            format: CertFormat::Der,
            data: PathOrBlob::Path(path.into()),
            password: password.into(),
        }
    }
}

impl SetOpt for PrivateKey {
    fn set_opt(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        easy.ssl_key_type(self.format.as_str())?;

        match &self.data {
            PathOrBlob::Path(path) => easy.ssl_key(path.as_path()),
            PathOrBlob::Blob(bytes) => unsafe {
                easy.setopt_blob_nocopy(CURLOPT_SSLKEY_BLOB, bytes)
            },
        }?;

        if let Some(password) = self.password.as_ref() {
            easy.key_password(password)?;
        }

        Ok(())
    }
}

impl SetOptProxy for PrivateKey {
    fn set_opt_proxy(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        easy.proxy_sslkey_type(self.format.as_str())?;

        match &self.data {
            PathOrBlob::Path(path) => easy.proxy_sslkey(path.to_str().unwrap()),
            PathOrBlob::Blob(bytes) => unsafe {
                easy.setopt_blob_nocopy(CURLOPT_PROXY_SSLKEY_BLOB, bytes)
            },
        }?;

        if let Some(password) = self.password.as_ref() {
            easy.proxy_key_password(password)?;
        }

        Ok(())
    }
}

/// curl supports both in-memory certs and certs loaded from files for mTLS.
/// This holds one or the other, depending on which the user has provided.
#[derive(Clone, Debug)]
enum PathOrBlob {
    Path(PathBuf),
    Blob(Arc<[u8]>),
}

/// Possible formats for certificates supported by curl.
#[derive(Clone, Copy, Debug)]
enum CertFormat {
    Pem,
    Der,
    Pkcs12,
}

impl CertFormat {
    fn as_str(&self) -> &'static str {
        match self {
            CertFormat::Pem => "PEM",
            CertFormat::Der => "DER",
            CertFormat::Pkcs12 => "P12",
        }
    }
}
