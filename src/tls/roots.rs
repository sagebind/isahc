//! Trusted root certificate discovery and handling.

use super::{has_tls_engine, Certificate, TlsEngine};
use crate::config::request::SetOpt;
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
/// store provided by the operating system (if any).
#[derive(Clone, Debug)]
pub struct RootCertStore(RootCertStoreImpl);

#[derive(Clone, Debug)]
enum RootCertStoreImpl {
    NoOp,
    Unset,
    File(PathBuf),
    Bundle(String),
    Rustls,
}

impl RootCertStore {
    /// Create an empty certificate store.
    ///
    /// Using this store will result in all server certificates being considered
    /// untrusted, and is generally useful only for testing.
    pub const fn empty() -> Self {
        Self(RootCertStoreImpl::Bundle(String::new()))
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
    /// This is normally the default certificate store used for most typical
    /// applications.
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
                return RootCertStore::file(path);
            }

            // These backends will use the store built into the OS as long as we
            // ensure no paths are set. They shouldn't be when curl is statically
            // linked, but they might be if using a system curl.
            if has_tls_engine(TlsEngine::Schannel) || has_tls_engine(TlsEngine::SecureTransport) {
                return RootCertStore(RootCertStoreImpl::Unset);
            }

            // When using rustls, we have to load native certs ourselves,
            // and a couple possible ways are available to us.
            if has_tls_engine(TlsEngine::Rustls) {
                return RootCertStore(RootCertStoreImpl::Rustls);
            }

            // If all else fails, use whatever curl wants to fall back to, if any.
            RootCertStore(RootCertStoreImpl::NoOp)
        });

        NATIVE_STORE.clone()
    }

    /// Use a file containing a bundle of certificates in PEM format.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::tls::RootCertStore;
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
    /// This store is supported by most TLS backends, including OpenSSL, rustls,
    /// and Secure Transport.
    pub fn custom<I>(certificates: I) -> Self
    where
        I: IntoIterator<Item = Certificate>,
    {
        // Generate a PEM bundle.
        let bundle = certificates.into_iter().map(|cert| cert.pem).collect();

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
            RootCertStoreImpl::NoOp => Ok(()),
            RootCertStoreImpl::File(path) => easy.cainfo(path),
            RootCertStoreImpl::Bundle(bundle) => easy.ssl_cainfo_blob(bundle.as_bytes()),

            #[allow(unsafe_code)]
            RootCertStoreImpl::Unset => {
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

            // If rustls-native-certs is enabled then we use it to access the
            // native store on-demand.
            #[cfg(feature = "rustls-native-certs")]
            RootCertStoreImpl::Rustls => {
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
                    Err(e) => {
                        let mut error = curl::Error::new(curl_sys::CURLE_SSL_CACERT_BADFILE);
                        error.set_extra(e.to_string());
                        Err(error)
                    }
                }
            }

            // If rustls-native-certs is not enabled then it is not possible to
            // use the native store.
            #[cfg(not(feature = "rustls-native-certs"))]
            RootCertStoreImpl::Rustls => Err(curl::Error::new(curl_sys::CURLE_SSL_CACERT_BADFILE)),
        }
    }
}
