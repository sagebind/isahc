//! Internal traits that define the Isahc configuration system.

use super::*;
use curl::easy::Easy2;

/// Configuration for an HTTP request.
///
/// This struct is not exposed directly, but rather is interacted with via the
/// [`Configurable`] trait.
#[derive(Clone, Debug, Default)]
pub struct RequestConfig {
    pub(crate) timeout: Option<Duration>,
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) version_negotiation: Option<VersionNegotiation>,
    pub(crate) redirect_policy: Option<RedirectPolicy>,
    pub(crate) auto_referer: Option<bool>,
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
    pub(crate) title_case_headers: bool,
    pub(crate) enable_metrics: Option<bool>,
}

/// Base trait for any object that can be configured for requests, such as an
/// HTTP request builder or an HTTP client.
#[doc(hidden)]
pub trait ConfigurableBase: Sized {
    /// Configure this object with the given property, returning the configured
    /// self.
    fn configure(self, property: impl Send + Sync + 'static) -> Self;

    fn with_config(self, f: impl FnOnce(&mut RequestConfig)) -> Self;
}

/// A helper trait for applying a configuration value to a given curl handle.
pub(crate) trait SetOpt {
    /// Apply this configuration property to the given curl handle.
    #[doc(hidden)]
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error>;
}
