//! Trusted root certificate discovery and handling.

use super::{has_tls_engine, Certificate, TlsEngine};
use crate::{
    config::{proxy::SetOptProxy, request::SetOpt},
    error::create_curl_error,
};
use curl::easy::Easy2;
use once_cell::sync::Lazy;
use std::{env, os::raw::c_char, path::PathBuf, ptr};

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
#[derive(Clone, Debug)]
pub struct RootCertStore(StoreImpl);

#[derive(Clone, Debug)]
enum StoreImpl {
    NoOp,
    Unset,
    FilePath(PathBuf),
    PemBundle(String),

    #[cfg(feature = "rustls-tls-native-certs")]
    RustlsNativeTls,
}

impl RootCertStore {
    /// Create an empty certificate store.
    ///
    /// Using this store will result in all server certificates being considered
    /// untrusted, and is generally useful only for testing.
    pub const fn empty() -> Self {
        Self(StoreImpl::PemBundle(String::new()))
    }

    /// Use the platform's native certificate store, if any.
    ///
    /// This is normally the default certificate store used for most typical
    /// applications.
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
    /// though naturally you will likely encounter certificate errors since the
    /// store will basically behave like [`RootCertStore::empty`].
    pub fn native() -> Self {
        /// To determine how to access the native store we have to perform some
        /// runtime checks and probing, so we only do this once and cache the
        /// result.
        static NATIVE_STORE: Lazy<RootCertStore> = Lazy::new(|| {
            // Ensure curl (and if applicable, openssl-probe) are initialized
            // before doing anything.
            curl::init();

            // If the `SSL_CERT_FILE` environment variable is set, use that.
            //
            // Note: OpenSSL checks `SSL_CERT_FILE` by default, but LibreSSL doesn't.
            // This makes the behavior consistent between the two. It also means that if
            // openssl-probe (which the curl crate may run during initialization)
            // discovered a cert file to use using its discovery mechanism, we will use
            // it even with LibreSSL even though openssl-probe doesn't work with
            // LibreSSL out of the box.
            if let Some(path) = env::var_os("SSL_CERT_FILE") {
                return RootCertStore::from_file(path);
            }

            // These backends will use the store built into the OS as long as we
            // ensure no paths are set. They shouldn't be when curl is statically
            // linked, but they might be if using a system curl.
            if has_tls_engine(TlsEngine::Schannel) || has_tls_engine(TlsEngine::SecureTransport) {
                return RootCertStore(StoreImpl::Unset);
            }

            // Use the rustls-native-certs crate if enabled. Note this already
            // implies using a statically-linked curl with rustls.
            #[cfg(feature = "rustls-tls-native-certs")]
            {
                RootCertStore(StoreImpl::RustlsNativeTls)
            }

            // If all else fails, use whatever curl wants to fall back to, if any.
            #[cfg(not(feature = "rustls-tls-native-certs"))]
            {
                RootCertStore(StoreImpl::NoOp)
            }
        });

        NATIVE_STORE.clone()
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
    /// use isahc::tls::RootCertStore;
    ///
    /// let store = RootCertStore::from_file("/etc/certs/cabundle.pem");
    /// ```
    pub fn from_file<P: Into<PathBuf>>(path: P) -> Self {
        Self(StoreImpl::FilePath(path.into()))
    }

    /// Create a custom certificate store containing exactly the given
    /// certificates.
    ///
    /// Server certificates will be verified using only certificates given.
    ///
    /// This store is supported by most TLS backends, including OpenSSL, rustls,
    /// and Secure Transport.
    pub fn custom<I>(certificates: I) -> Self
    where
        I: IntoIterator<Item = Certificate>,
    {
        // Generate a PEM bundle.
        let bundle = certificates.into_iter().map(|cert| cert.pem).collect();

        Self(StoreImpl::PemBundle(bundle))
    }
}

impl Default for RootCertStore {
    fn default() -> Self {
        // If we were configured to use rustls without native cert support, use
        // an empty store as the default.
        //
        // Note that if we are dynamically linked to a curl that was built using
        // rustls, then it is still possible to use the native store if that curl
        // was built that way.
        if cfg!(feature = "rustls-tls") && !cfg!(feature = "rustls-tls-native-certs") {
            Self(StoreImpl::NoOp)
        } else {
            Self::native()
        }
    }
}

impl From<Certificate> for RootCertStore {
    fn from(certificate: Certificate) -> Self {
        Self::custom([certificate])
    }
}

impl SetOpt for RootCertStore {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match &self.0 {
            StoreImpl::NoOp => Ok(()),
            StoreImpl::FilePath(path) => easy.cainfo(path),
            StoreImpl::PemBundle(bundle) => easy.ssl_cainfo_blob(bundle.as_bytes()),

            #[allow(unsafe_code)]
            StoreImpl::Unset => {
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

            #[cfg(feature = "rustls-tls-native-certs")]
            StoreImpl::RustlsNativeTls => {
                use once_cell::sync::OnceCell;

                static NATIVE_CERTS: OnceCell<RootCertStore> = OnceCell::new();

                fn load_native_certs() -> std::io::Result<RootCertStore> {
                    Ok(RootCertStore::custom(
                        rustls_native_certs::load_native_certs()?
                            .into_iter()
                            .map(|cert| Certificate::from_der(cert.0)),
                    ))
                }

                match NATIVE_CERTS.get_or_try_init(load_native_certs) {
                    Ok(certs) => certs.set_opt(easy),
                    Err(e) => Err(create_curl_error(curl_sys::CURLE_SSL_CACERT_BADFILE, e)),
                }
            }
        }
    }
}

impl SetOptProxy for RootCertStore {
    fn set_opt_proxy<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match &self.0 {
            StoreImpl::NoOp => Ok(()),
            StoreImpl::FilePath(path) => {
                if let Some(path) = path.to_str() {
                    easy.proxy_cainfo(path)
                } else {
                    Err(create_curl_error(
                        curl_sys::CURLE_SSL_CACERT_BADFILE,
                        "path is not valid UTF-8",
                    ))
                }
            }
            StoreImpl::PemBundle(bundle) => easy.proxy_ssl_cainfo_blob(bundle.as_bytes()),

            #[allow(unsafe_code)]
            StoreImpl::Unset => {
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

            #[cfg(feature = "rustls-tls-native-certs")]
            StoreImpl::RustlsNativeTls => {
                use once_cell::sync::OnceCell;

                static NATIVE_CERTS: OnceCell<RootCertStore> = OnceCell::new();

                fn load_native_certs() -> std::io::Result<RootCertStore> {
                    Ok(RootCertStore::custom(
                        rustls_native_certs::load_native_certs()?
                            .into_iter()
                            .map(|cert| Certificate::from_der(cert.0)),
                    ))
                }

                match NATIVE_CERTS.get_or_try_init(load_native_certs) {
                    Ok(certs) => certs.set_opt(easy),
                    Err(e) => Err(create_curl_error(curl_sys::CURLE_SSL_CACERT_BADFILE, e)),
                }
            }
        }
    }
}
