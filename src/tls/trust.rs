//! This module provides the API used to configure how trust of TLS server
//! certificates and certificate authorities should be determined.
//!
//! This API is called "trust" because it might not be as simple as a list of
//! root certificates. You can delegate to other verifier APIs to determine
//! which certificates you trust, for example.

use super::TlsEngine;
use crate::{
    config::setopt::{SetOpt, SetOptError, SetOptProxy},
    error::{Error, ErrorKind},
    info::curl_version,
    util::set_blob_nocopy,
};
use curl::easy::{Easy2, SslOpt};
use curl_sys::{CURL_BLOB_NOCOPY, curl_blob};
use std::{
    env, fmt,
    os::raw::c_char,
    path::PathBuf,
    ptr,
    sync::{Arc, LazyLock},
};

/// A store that provides a collection of trusted root certificates.
///
/// Root certificates are used for validating the authenticity of a server
/// before proceeding with a request. If the server presents a certificate that
/// matches the server's information, and is signed by a certificate authority
/// either in the root certificate store or is itself trusted by another
/// certificate in the store, then the server is considered to be legitimate.
///
/// Isahc supports multiple kinds of stores, though the default is to use the
/// shared store provided by the operating system (if any).
///
/// # Defaults
///
/// The default trust store that is used (and returned by the [`Default`]
/// implementation) depends on how Isahc was compiled and the environment it is
/// running in. With the default crate feature set, [`TrustStore::native`] is
/// the default implementation, which will use the operating system's shared
/// certificate store.
///
/// If the `rustls-tls-webpki-roots` feature is enabled, then the default is to
/// use [`TrustStore::webpki_roots`] instead.
///
/// # Cloning
///
/// A trust store can be expensive to create, but once created, it should be
/// considered cheap to clone. This allows you to easily reuse it across
/// multiple requests or multiple HTTP clients.
#[derive(Clone)]
pub struct TrustStore(Repr);

#[derive(Clone)]
enum Repr {
    /// Do nothing to configure trust, and rely on curl's default behavior.
    NoOp,

    // Unset CA-related options in case they are set by default. Usually this is
    // a way of asking to use the OS root certificate store for certain
    // backends.
    Unset,

    /// Sets the `CURLSSLOPT_NATIVE_CA` flag, which asks curl to use the OS
    /// native CA store, if possible and supported by the current TLS backend.
    ///
    /// When using rustls, this will cause curl to use the
    /// rustls-platform-verifier crate (via rustls-ffi) to verify any certs.
    ///
    /// See <https://curl.se/libcurl/c/CURLOPT_SSL_OPTIONS.html>.
    NativeCa,

    /// Use a certificate bundle from a file path.
    ///
    /// This may or may not be combined with OS-native certificate stores,
    /// depending on the TLS backend.
    FilePath(PathBuf),

    /// Use an in-memory bundle of certificates.
    ///
    /// This may or may not be combined with OS-native certificate stores,
    /// depending on the TLS backend.
    ///
    /// Since the bundle could be very large, it would be extremely wasteful to
    /// have curl copy the bundle into its own memory every time a request is
    /// made. So to deal with this, we use the C API to allocate a single "blob"
    /// that can be reused with multiple parallel easy handles and does not need
    /// to be copied.
    ///
    /// This requires some `unsafe` because we must be very careful to ensure
    /// this blob is not freed until it is no longer in use.
    PemBundle {
        // TODO: How to track when it is safe to free this?
        bytes: Arc<[u8]>,
    },
}

impl TrustStore {
    /// Use the operating system's native APIs for verifying certificate trust,
    /// if possible. This is normally the trust method used for most typical applications.
    ///
    /// On Windows, macOS, and iOS this involves using the certificate
    /// management features provided by the operating system. On Linux and other
    /// UNIX-like systems this typically will use a shared certificate bundle
    /// managed by the distribution or system administrator. In most cases, this
    /// will also respect environment variables that override where to look for
    /// trusted certificates.
    ///
    /// # Error handling
    ///
    /// The presence or ability to access a system certificate store is not
    /// checked here. If the system store cannot be accessed due to permissions
    /// or some other kind of problem, an error will be returned when attempting
    /// to send a request using the store.
    ///
    /// If the system store is simply empty or at least *appears* to be empty,
    /// the TLS backend will probably not consider this an inherent error,
    /// though naturally you will likely encounter certificate errors since no
    /// certificates will be considered trusted.
    pub fn native() -> Self {
        /// To determine how to access the native store we have to perform some
        /// runtime checks and probing, so we only do this once and cache the
        /// result.
        static NATIVE_STORE: LazyLock<TrustStore> = LazyLock::new(TrustStore::new_native);

        NATIVE_STORE.clone()
    }

    fn new_native() -> Self {
        // Ensure curl (and if applicable, openssl-probe) are initialized
        // before doing anything.
        curl::init();

        // If the `SSL_CERT_FILE` environment variable is set, use that. At
        // least in a unix-like environment, doing so is considered common
        // courtesy.
        //
        // Note: OpenSSL checks `SSL_CERT_FILE` by default so we wouldn't need
        // to do this, but LibreSSL doesn't. This makes the behavior consistent
        // between the two. It also means that if openssl-probe (which the curl
        // crate may run during initialization) discovered a cert file to use
        // using its discovery mechanism, we will use it even with LibreSSL even
        // though openssl-probe doesn't work with LibreSSL out of the box.
        if let Some(path) = env::var_os("SSL_CERT_FILE") {
            if !path.is_empty() {
                tracing::debug!(
                    ?path,
                    "using certificate bundle from SSL_CERT_FILE environment variable",
                );
                return TrustStore::from_file(path);
            }
        }

        // If we are using curl 8.13.0+ with rustls, then we can ask for
        // rustls-platform-verifier to be used to verify server certificates
        // using the OS-native certificate store.
        if TlsEngine::Rustls.is_available() && curl_version() >= (8, 13, 0) {
            tracing::debug!("using platform verifier with rustls");
            return TrustStore(Repr::NativeCa);
        }

        // These backends will use the store built into the OS as long as we
        // ensure no paths are set. They shouldn't be when curl is statically
        // linked, but they might be if using a system curl.
        if TlsEngine::Schannel.is_available() || TlsEngine::SecureTransport.is_available() {
            return TrustStore(Repr::Unset);
        }

        // If all else fails, use whatever curl wants to fall back to, if any.
        TrustStore(Repr::NoOp)
    }

    /// Use a file containing a bundle of certificates in PEM format.
    ///
    /// The certificate bundle is not loaded or validated here. If the file does
    /// not exist or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending bundle.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::tls::TrustStore;
    ///
    /// let store = TrustStore::from_file("/etc/certs/cabundle.pem");
    /// ```
    pub fn from_file<P: Into<PathBuf>>(path: P) -> Self {
        Self(Repr::FilePath(path.into()))
    }

    /// Create a custom certificate store containing exactly the given
    /// certificates.
    ///
    /// Server certificates will be verified using only certificates given.
    ///
    /// This store is supported by most TLS backends, including OpenSSL, rustls,
    /// and Secure Transport.
    pub fn custom() -> CertificateBundleBuilder {
        CertificateBundleBuilder { pem: Vec::new() }
    }

    pub(super) fn configure_ssl_options(&self, ssl_opt: &mut SslOpt) {
        if let Repr::NativeCa = &self.0 {
            ssl_opt.native_ca(true);
        }
    }
}

impl Default for TrustStore {
    #[cfg(feature = "rustls-tls-webpki-roots")]
    fn default() -> Self {
        tracing::debug!("using webpki_roots as default trust store");
        Self::webpki_roots()
    }

    #[cfg(not(feature = "rustls-tls-webpki-roots"))]
    fn default() -> Self {
        Self::native()
    }
}

impl SetOpt for TrustStore {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError> {
        match &self.0 {
            Repr::NativeCa | Repr::NoOp => Ok(()),
            Repr::FilePath(path) => easy.cainfo(path).map_err(Into::into),
            Repr::PemBundle { bytes } => {
                unsafe { set_blob_nocopy(easy, curl_sys::CURLOPT_CAINFO_BLOB, bytes) }
                    .map_err(Into::into)
            }
            Repr::Unset => {
                // safe wrapper does not allow setting to null
                unsafe {
                    curl_sys::curl_easy_setopt(
                        easy.raw(),
                        curl_sys::CURLOPT_CAINFO,
                        ptr::null_mut::<c_char>(),
                    );
                    curl_sys::curl_easy_setopt(
                        easy.raw(),
                        curl_sys::CURLOPT_CAPATH,
                        ptr::null_mut::<c_char>(),
                    );
                }

                Ok(())
            }
        }
    }
}

impl SetOptProxy for TrustStore {
    fn set_opt_proxy<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError> {
        match &self.0 {
            Repr::NativeCa | Repr::NoOp => Ok(()),
            Repr::FilePath(path) => {
                if let Some(path) = path.to_str() {
                    easy.proxy_cainfo(path).map_err(Into::into)
                } else {
                    Err(Error::new(
                        ErrorKind::InvalidTlsConfiguration,
                        CertificatePathNotUtf8Error { path: path.clone() },
                    )
                    .into())
                }
            }
            Repr::PemBundle { bytes } => {
                unsafe { set_blob_nocopy(easy, curl_sys::CURLOPT_PROXY_CAINFO_BLOB, bytes) }
                    .map_err(Into::into)
            }
            Repr::Unset => {
                // safe wrapper does not allow setting to null
                unsafe {
                    curl_sys::curl_easy_setopt(
                        easy.raw(),
                        curl_sys::CURLOPT_PROXY_CAINFO,
                        ptr::null_mut::<c_char>(),
                    );
                    curl_sys::curl_easy_setopt(
                        easy.raw(),
                        curl_sys::CURLOPT_PROXY_CAPATH,
                        ptr::null_mut::<c_char>(),
                    );
                }

                Ok(())
            }
        }
    }
}

impl fmt::Debug for TrustStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::NativeCa => f.debug_tuple("TrustStore::NativeCa").finish(),
            Repr::FilePath(path) => f.debug_tuple("TrustStore::FilePath").field(path).finish(),
            Repr::PemBundle { .. } => f.debug_tuple("TrustStore::PemBundle").finish(),
            Repr::Unset => f.debug_tuple("TrustStore::Unset").finish(),
            Repr::NoOp => f.debug_tuple("TrustStore::NoOp").finish(),
        }
    }
}

/// Builds a custom bundle of X.509 certificates for certificate authorities
/// that are considered trusted for verifying server certificates.
#[derive(Clone, Debug)]
pub struct CertificateBundleBuilder {
    pem: Vec<u8>,
}

impl CertificateBundleBuilder {
    /// Add a trusted certificate in PEM format.
    ///
    /// The certificates are not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn add_from_pem(mut self, pem: &str) -> Self {
        self.pem.extend_from_slice(pem.as_bytes());
        self
    }

    /// Add a trusted certificate in DER format.
    ///
    /// The certificates are not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn add_from_der(mut self, der: &[u8]) -> Self {
        let label = "CERTIFICATE";
        let line_ending = Default::default();

        let len = pem_rfc7468::encoded_len(label, line_ending, der).unwrap();
        let existing_len = self.pem.len();

        self.pem.resize(existing_len + len, 0);

        pem_rfc7468::encode(label, line_ending, der, &mut self.pem[existing_len..]).unwrap();

        self
    }

    pub fn build(self) -> TrustStore {
        TrustStore(Repr::PemBundle {
            bytes: self.pem.into(),
        })
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
    pub fn from_der<B: AsRef<[u8]>>(der: B) -> Self {
        let pem =
            pem_rfc7468::encode_string("CERTIFICATE", Default::default(), der.as_ref()).unwrap();

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

    pub(crate) fn as_pem_bytes(&self) -> &[u8] {
        self.pem.as_bytes()
    }
}

#[derive(Clone, Debug)]
struct CertificatePathNotUtf8Error {
    path: PathBuf,
}

impl std::error::Error for CertificatePathNotUtf8Error {}

impl fmt::Display for CertificatePathNotUtf8Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "certificate path is not valid UTF-8: {}",
            self.path.display()
        )
    }
}
