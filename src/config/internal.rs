//! Internal traits that define the Isahc configuration system.

use super::*;
use curl::easy::Easy2;

/// Base trait for any object that can be configured for requests, such as an
/// HTTP request builder or an HTTP client.
#[doc(hidden)]
pub trait ConfigurableBase: Sized {
    /// Invoke a function to mutate the request configuration for this object.
    fn with_config(self, f: impl FnOnce(&mut RequestConfig)) -> Self;
}

/// A helper trait for applying a configuration value to a given curl handle.
pub(crate) trait SetOpt {
    /// Apply this configuration property to the given curl handle.
    #[doc(hidden)]
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error>;
}

/// Configuration for an HTTP request.
///
/// This struct is not exposed directly, but rather is interacted with via the
/// [`Configurable`] trait.
#[derive(Clone, Debug, Default, merge::Merge)]
pub struct RequestConfig {
    pub(crate) timeout: Option<Duration>,
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) version_negotiation: Option<VersionNegotiation>,
    pub(crate) redirect_policy: Option<RedirectPolicy>,
    pub(crate) auto_referer: Option<bool>,

    /// Enable or disable automatically decompressing the response body.
    pub(crate) automatic_decompression: Option<bool>,
    pub(crate) authentication: Option<Authentication>,
    pub(crate) credentials: Option<Credentials>,
    pub(crate) tcp_keepalive: Option<Duration>,
    pub(crate) tcp_nodelay: Option<bool>,
    pub(crate) interface: Option<NetworkInterface>,
    pub(crate) ip_version: Option<IpVersion>,
    pub(crate) dial: Option<Dialer>,
    pub(crate) proxy: Option<Option<http::Uri>>,
    pub(crate) proxy_blacklist: Option<proxy::Blacklist>,
    pub(crate) proxy_authentication: Option<Authentication>,
    pub(crate) proxy_credentials: Option<Credentials>,
    pub(crate) max_upload_speed: Option<u64>,
    pub(crate) max_download_speed: Option<u64>,
    pub(crate) ssl_client_certificate: Option<ClientCertificate>,
    pub(crate) ssl_ca_certificate: Option<CaCertificate>,
    pub(crate) ssl_ciphers: Option<ssl::Ciphers>,
    pub(crate) ssl_options: Option<SslOption>,

    /// Send header names as title case instead of lowercase.
    pub(crate) title_case_headers: Option<bool>,
    pub(crate) enable_metrics: Option<bool>,
}

impl RequestConfig {
    #[inline]
    pub(crate) fn get<'a, T, F>(&'a self, overrides: Option<&'a Self>, f: F) -> Option<T>
    where
        T: 'a,
        F: Fn(&'a Self) -> Option<T> + 'a,
    {
        overrides.and_then(|c| f(c)).or_else(|| f(self))
    }
}

impl SetOpt for RequestConfig {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        eprintln!("{:?}", self);

        if let Some(timeout) = self.timeout {
            easy.timeout(timeout)?;
        }

        if let Some(timeout) = self.connect_timeout {
            easy.connect_timeout(timeout)?;
        }

        if let Some(negotiation) = self.version_negotiation.as_ref() {
            negotiation.set_opt(easy)?;
        }

        #[allow(unsafe_code)]
        if let Some(enable) = self.automatic_decompression {
            if enable {
                // Enable automatic decompression, and also populate the
                // Accept-Encoding header with all supported encodings if not
                // explicitly set.
                easy.accept_encoding("")?;
            } else {
                // Use raw FFI because safe wrapper doesn't let us set to null.
                unsafe {
                    match curl_sys::curl_easy_setopt(easy.raw(), curl_sys::CURLOPT_ACCEPT_ENCODING, 0) {
                        curl_sys::CURLE_OK => {},
                        code => return Err(curl::Error::new(code)),
                    }
                }
            }
        }

        if let Some(auth) = self.authentication.as_ref() {
            auth.set_opt(easy)?;
        }

        if let Some(credentials) = self.credentials.as_ref() {
            credentials.set_opt(easy)?;
        }

        if let Some(interval) = self.tcp_keepalive {
            easy.tcp_keepalive(true)?;
            easy.tcp_keepintvl(interval)?;
        }

        if let Some(enable) = self.tcp_nodelay {
            easy.tcp_nodelay(enable)?;
        }

        if let Some(max) = self.max_upload_speed {
            easy.max_send_speed(max)?;
        }

        if let Some(max) = self.max_download_speed {
            easy.max_recv_speed(max)?;
        }

        if let Some(enable) = self.enable_metrics {
            easy.progress(enable)?;
        }

        if let Some(version) = self.ip_version.as_ref() {
            easy.ip_resolve(match version {
                IpVersion::V4 => curl::easy::IpResolve::V4,
                IpVersion::V6 => curl::easy::IpResolve::V6,
                IpVersion::Any => curl::easy::IpResolve::Any,
            })?;
        }

        if let Some(dialer) = self.dial.as_ref() {
            dialer.set_opt(easy)?;
        }

        if let Some(proxy) = self.proxy.as_ref() {
            match proxy {
                Some(uri) => easy.proxy(&format!("{}", uri))?,
                None => easy.proxy("")?,
            }
        }

        if let Some(blacklist) = self.proxy_blacklist.as_ref() {
            blacklist.set_opt(easy)?;
        }

        if let Some(auth) = self.proxy_authentication.as_ref() {
            #[cfg(feature = "spnego")]
            {
                if auth.contains(Authentication::negotiate()) {
                    // Ensure auth engine is enabled, even though credentials do not
                    // need to be specified.
                    easy.proxy_username("")?;
                    easy.proxy_password("")?;
                }
            }

            easy.proxy_auth(&auth.as_auth())?;
        }

        if let Some(credentials) = self.proxy_credentials.as_ref() {
            easy.proxy_username(&credentials.username)?;
            easy.proxy_password(&credentials.password)?;
        }

        Ok(())
    }
}
