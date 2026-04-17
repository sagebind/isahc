//! Trusted root certificate discovery and handling.

use super::{Certificate, TlsEngine, has_tls_engine};
use crate::{
    config::setopt::{SetOpt, SetOptError, SetOptProxy},
    error::create_curl_error,
    info::curl_version,
};
use curl::easy::{Easy2, SslOpt};
use std::{env, os::raw::c_char, path::PathBuf, ptr, sync::LazyLock};

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
pub struct RootCertStore(TrustConfigurer);

#[derive(Clone, Debug)]
enum TrustConfigurer {
    NoOp,
    Unset,
    FilePath(PathBuf),
    PemBundle(String),
    NativeCa,

    #[cfg(feature = "rustls-tls-native-certs")]
    RustlsNativeTls,
}

impl RootCertStore {
    /// Create an empty certificate store.
    ///
    /// Using this store will result in all server certificates being considered
    /// untrusted, and is generally useful only for testing.
    pub const fn empty() -> Self {
        Self(TrustConfigurer::PemBundle(String::new()))
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
        static NATIVE_STORE: LazyLock<RootCertStore> = LazyLock::new(RootCertStore::new_native);

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
                    "using certificate file from SSL_CERT_FILE environment variable: {:?}",
                    path
                );
                return RootCertStore::from_file(path);
            }
        }

        // These backends will use the store built into the OS as long as we
        // ensure no paths are set. They shouldn't be when curl is statically
        // linked, but they might be if using a system curl.
        if has_tls_engine(TlsEngine::Schannel) || has_tls_engine(TlsEngine::SecureTransport) {
            return RootCertStore(TrustConfigurer::Unset);
        }

        // If we are using curl 8.13.0+ with rustls, then we can ask for
        // rustls-platform-verifier to be used to verify server certificates
        // using the OS-native certificate store.
        if has_tls_engine(TlsEngine::Rustls) && curl_version() >= (8, 13, 0) {
            tracing::debug!("using platform verifier with rustls");
            return RootCertStore(TrustConfigurer::NativeCa);
        }

        // Use the rustls-native-certs crate if enabled. Note this already
        // implies using a statically-linked curl with rustls.
        #[cfg(feature = "rustls-tls-native-certs")]
        {
            RootCertStore(TrustConfigurer::RustlsNativeTls)
        }

        // If all else fails, use whatever curl wants to fall back to, if any.
        #[cfg(not(feature = "rustls-tls-native-certs"))]
        {
            RootCertStore(TrustConfigurer::NoOp)
        }
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
        Self(TrustConfigurer::FilePath(path.into()))
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
        let bundle = certificates
            .into_iter()
            .map(|cert| cert.into_pem_string())
            .collect();

        Self(TrustConfigurer::PemBundle(bundle))
    }

    pub(super) fn configure_ssl_options(&self, ssl_opt: &mut SslOpt) {
        if let TrustConfigurer::NativeCa = &self.0 {
            ssl_opt.native_ca(true);
        }
    }
}

impl Default for RootCertStore {
    fn default() -> Self {
        // If we were configured to use rustls without native cert support, use
        // an empty store as the default.
        //
        // Note that if we are dynamically linked to a curl that was built using
        // rustls, then it is still possible to use the native store if that
        // curl was built that way.
        if cfg!(feature = "rustls-tls")
            && !cfg!(feature = "rustls-tls-native-certs")
            && !cfg!(feature = "rustls-tls-platform-verifier")
        {
            Self(TrustConfigurer::NoOp)
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
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError> {
        match &self.0 {
            TrustConfigurer::FilePath(path) => easy.cainfo(path).map_err(Into::into),
            TrustConfigurer::PemBundle(bundle) => {
                easy.ssl_cainfo_blob(bundle.as_bytes()).map_err(Into::into)
            }

            #[allow(unsafe_code)]
            TrustConfigurer::Unset => {
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
            #[allow(unsafe_code)]
            TrustConfigurer::RustlsNativeTls => {
                // let mut result = rustls_native_certs::load_native_certs();

                // if let Some(e) = result.errors.pop() {
                //     return Err(create_curl_error(curl_sys::CURLE_SSL_CACERT_BADFILE, e).into());
                // }

                // RootCertStore::custom(
                //     result
                //         .certs
                //         .into_iter()
                //         .map(|cert| Certificate::from_der(cert)),
                // )
                // .set_opt(easy)
                Ok(())
            }

            _ => Ok(()),
        }
    }
}

impl SetOptProxy for RootCertStore {
    fn set_opt_proxy<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError> {
        match &self.0 {
            TrustConfigurer::FilePath(path) => {
                if let Some(path) = path.to_str() {
                    easy.proxy_cainfo(path).map_err(Into::into)
                } else {
                    Err(create_curl_error(
                        curl_sys::CURLE_SSL_CACERT_BADFILE,
                        "path is not valid UTF-8",
                    )
                    .into())
                }
            }
            TrustConfigurer::PemBundle(bundle) => easy
                .proxy_ssl_cainfo_blob(bundle.as_bytes())
                .map_err(Into::into),

            #[allow(unsafe_code)]
            TrustConfigurer::Unset => {
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
            TrustConfigurer::RustlsNativeTls => {
                let mut result = rustls_native_certs::load_native_certs();

                if let Some(e) = result.errors.pop() {
                    return Err(create_curl_error(curl_sys::CURLE_SSL_CACERT_BADFILE, e).into());
                }

                RootCertStore::custom(
                    result
                        .certs
                        .into_iter()
                        .map(|cert| Certificate::from_der(cert)),
                )
                .set_opt_proxy(easy)
            }

            _ => Ok(()),
        }
    }
}
