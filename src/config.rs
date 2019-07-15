//! Definition of all client and request configuration options.
//!
//! Individual options are separated out into multiple types. Each type acts
//! both as a "field name" and the value of that option.

use std::iter::FromIterator;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Describes a policy for handling server redirects.
///
/// The default is to not follow redirects.
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
    /// A PEM-encoded private key file.
    PEM {
        /// Path to the key file.
        path: PathBuf,

        /// Password to decrypt the key file.
        password: Option<String>,
    },
    /// A DER-encoded private key file.
    DER {
        /// Path to the key file.
        path: PathBuf,

        /// Password to decrypt the key file.
        password: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Timeout(pub(crate) Duration);

#[derive(Clone, Debug)]
pub(crate) struct ConnectTimeout(pub(crate) Duration);

#[derive(Clone, Debug)]
pub(crate) struct TcpKeepAlive(pub(crate) Duration);

#[derive(Clone, Debug)]
pub(crate) struct PreferredHttpVersion(pub(crate) http::Version);

#[derive(Clone, Copy, Debug)]
pub(crate) struct TcpNoDelay;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AutoReferer;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxUploadSpeed(pub(crate) u64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxDownloadSpeed(pub(crate) u64);

#[derive(Clone, Debug)]
pub(crate) struct DnsServers(pub(crate) Vec<SocketAddr>);

impl FromIterator<SocketAddr> for DnsServers {
    fn from_iter<I: IntoIterator<Item = SocketAddr>>(iter: I) -> Self {
        DnsServers(Vec::from_iter(iter))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Proxy(pub(crate) http::Uri);

#[derive(Clone, Debug)]
pub(crate) struct SslCiphers(pub(crate) Vec<String>);

impl FromIterator<String> for SslCiphers {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        SslCiphers(Vec::from_iter(iter))
    }
}
