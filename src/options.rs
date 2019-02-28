//! Definition of all configurable client options.

use std::path::PathBuf;
use std::time::Duration;

/// Defines various protocol and connection options.
#[derive(Clone, Debug, withers_derive::Withers)]
pub struct Options {
    /// The policy for automatically following server redirects.
    ///
    /// The default is to not follow redirects.
    pub redirect_policy: RedirectPolicy,

    /// A preferred HTTP version the client should attempt to use to communicate
    /// to the server with.
    ///
    /// This is treated as a suggestion. A different version may be used if the
    /// server does not support it or negotiates a different version.
    ///
    /// The default value is `None` (any version).
    pub preferred_http_version: Option<http::Version>,

    /// A timeout for the maximum time allowed for a request-response cycle.
    ///
    /// The default value is `None` (unlimited).
    pub timeout: Option<Duration>,

    /// A timeout for the initial connection phase.
    ///
    /// The default value is 300 seconds.
    pub connect_timeout: Duration,

    /// Enable or disable TCP keepalive with a given probe interval.
    ///
    /// The default value is `None` (disabled).
    pub tcp_keepalive: Option<Duration>,

    /// Enable or disable the `TCP_NODELAY` option.
    ///
    /// The default value is `false`.
    pub tcp_nodelay: bool,

    /// Set the max buffer size in bytes to use for reading the response body.
    ///
    /// The default value is 8 KiB.
    pub buffer_size: usize,

    /// Indicates whether the `Referer` header should be automatically updated.
    pub auto_referer: bool,

    /// A proxy to use for requests.
    ///
    /// The proxy protocol is specified by the URI scheme.
    ///
    /// - **`http`**: Proxy. Default when no scheme is specified.
    /// - **`https`**: HTTPS Proxy. (Added in 7.52.0 for OpenSSL, GnuTLS and NSS)
    /// - **`socks4`**: SOCKS4 Proxy.
    /// - **`socks4a`**: SOCKS4a Proxy. Proxy resolves URL hostname.
    /// - **`socks5`**: SOCKS5 Proxy.
    /// - **`socks5h`**: SOCKS5 Proxy. Proxy resolves URL hostname.
    pub proxy: Option<http::Uri>,

    /// A maximum upload speed for the request body, in bytes per second.
    ///
    /// The default is unlimited.
    pub max_upload_speed: Option<u64>,

    /// A maximum download speed for the response body, in bytes per second.
    ///
    /// The default is unlimited.
    pub max_download_speed: Option<u64>,

    /// A list of ciphers to use for SSL/TLS connections.
    ///
    /// The list of valid cipher names is dependent on the underlying SSL/TLS
    /// engine in use.
    ///
    /// You can find an up-to-date list of potential cipher names at
    /// <https://curl.haxx.se/docs/ssl-ciphers.html>.
    ///
    /// The default is unset and will result in the system defaults being used.
    pub ssl_ciphers: Option<Vec<String>>,

    /// A custom SSL/TLS client certificate to use for all client connections.
    ///
    /// If a format is not supported by the underlying SSL/TLS engine, an error
    /// will be returned when attempting to send a request using the offending
    /// certificate.
    ///
    /// The default value is none.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chttp::options::*;
    /// let cert = ClientCertificate::PEM {
    ///     path: "client.pem".into(),
    ///     private_key: Some(PrivateKey::PEM {
    ///         path: "key.pem".into(),
    ///         password: Some("secret".into()),
    ///     }),
    /// };
    /// let options = Options::default()
    ///     .with_ssl_client_certificate(Some(cert));
    /// ```
    pub ssl_client_certificate: Option<ClientCertificate>,
}

impl Default for Options {
    /// Create a new options with the default values.
    fn default() -> Self {
        Self {
            redirect_policy: RedirectPolicy::default(),
            preferred_http_version: None,
            timeout: None,
            connect_timeout: Duration::from_secs(300),
            tcp_keepalive: None,
            tcp_nodelay: false,
            buffer_size: 8192,
            auto_referer: false,
            proxy: None,
            max_upload_speed: None,
            max_download_speed: None,
            ssl_ciphers: None,
            ssl_client_certificate: None,
        }
    }
}

/// Describes a policy for handling server redirects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPolicy {
    /// Do not apply any special treatment to redirect responses. The response
    /// will be returned as-is and redirects will not be followed.
    ///
    /// This is the default policy.
    None,
    /// Follow all redirects automatically.
    Follow,
    /// Follow redirects automatically up to a maximum number of redirects.
    Limit(u32),
}

impl Default for RedirectPolicy {
    fn default() -> Self {
        RedirectPolicy::None
    }
}

/// A public key certificate file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientCertificate {
    /// A PEM-encoded certificate file.
    PEM {
        /// Path to the certificate file.
        path: PathBuf,

        /// Private key corresponding to the SSL/TLS certificate.
        private_key: Option<PrivateKey>,
    },
    /// A DER-encoded certificate file.
    DER {
        /// Path to the certificate file.
        path: PathBuf,

        /// Private key corresponding to the SSL/TLS certificate.
        private_key: Option<PrivateKey>,
    },
    /// A PKCS#12-encoded certificate file.
    P12 {
        /// Path to the certificate file.
        path: PathBuf,

        /// Password to decrypt the certificate file.
        password: Option<String>,
    },
}

/// A private key file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivateKey {
    PEM {
        /// Path to the key file.
        path: PathBuf,

        /// Password to decrypt the key file.
        password: Option<String>,
    },
    DER {
        /// Path to the key file.
        path: PathBuf,

        /// Password to decrypt the key file.
        password: Option<String>,
    },
}
