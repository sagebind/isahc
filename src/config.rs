//! Definition of all client and request configuration options.
//!
//! Individual options are separated out into multiple types. Each type acts
//! both as a "field name" and the value of that option.

use std::iter::FromIterator;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// A helper trait for applying a configuration value to a given curl handle.
pub(crate) trait SetOpt {
    /// Apply this configuration option to the given curl handle.
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error>;
}

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

impl SetOpt for RedirectPolicy {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        match self {
            RedirectPolicy::Follow => {
                easy.follow_location(true)?;
            }
            RedirectPolicy::Limit(max) => {
                easy.follow_location(true)?;
                easy.max_redirections(*max)?;
            }
            RedirectPolicy::None => {
                easy.follow_location(false)?;
            }
        }

        Ok(())
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

impl SetOpt for ClientCertificate {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        match self {
            ClientCertificate::PEM { path, private_key } => {
                easy.ssl_cert(path)?;
                easy.ssl_cert_type("PEM")?;
                if let Some(key) = private_key {
                    key.set_opt(easy)?;
                }
            }
            ClientCertificate::DER { path, private_key } => {
                easy.ssl_cert(path)?;
                easy.ssl_cert_type("DER")?;
                if let Some(key) = private_key {
                    key.set_opt(easy)?;
                }
            }
            ClientCertificate::P12 { path, password } => {
                easy.ssl_cert(path)?;
                easy.ssl_cert_type("P12")?;
                if let Some(password) = password {
                    easy.key_password(password)?;
                }
            }
        }

        Ok(())
    }
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

impl SetOpt for PrivateKey {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        match self {
            PrivateKey::PEM { path, password } => {
                easy.ssl_key(path)?;
                easy.ssl_key_type("PEM")?;
                if let Some(password) = password {
                    easy.key_password(password)?;
                }
            }
            PrivateKey::DER { path, password } => {
                easy.ssl_key(path)?;
                easy.ssl_key_type("DER")?;
                if let Some(password) = password {
                    easy.key_password(password)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Timeout(pub(crate) Duration);

impl SetOpt for Timeout {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.timeout(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ConnectTimeout(pub(crate) Duration);

impl SetOpt for ConnectTimeout {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.connect_timeout(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TcpKeepAlive(pub(crate) Duration);

impl SetOpt for TcpKeepAlive {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.tcp_keepalive(true)?;
        easy.tcp_keepintvl(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PreferredHttpVersion(pub(crate) http::Version);

impl SetOpt for PreferredHttpVersion {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.http_version(match self.0 {
            http::Version::HTTP_10 => curl::easy::HttpVersion::V10,
            http::Version::HTTP_11 => curl::easy::HttpVersion::V11,
            http::Version::HTTP_2 => curl::easy::HttpVersion::V2,
            _ => curl::easy::HttpVersion::Any,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TcpNoDelay;

impl SetOpt for TcpNoDelay {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.tcp_nodelay(true)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AutoReferer;

impl SetOpt for AutoReferer {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.autoreferer(true)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxUploadSpeed(pub(crate) u64);

impl SetOpt for MaxUploadSpeed {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.max_send_speed(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaxDownloadSpeed(pub(crate) u64);

impl SetOpt for MaxDownloadSpeed {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.max_recv_speed(self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DnsServers(pub(crate) Vec<SocketAddr>);

impl FromIterator<SocketAddr> for DnsServers {
    fn from_iter<I: IntoIterator<Item = SocketAddr>>(iter: I) -> Self {
        DnsServers(Vec::from_iter(iter))
    }
}

impl SetOpt for DnsServers {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        let dns_string = self.0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        // DNS servers should not be hard error.
        if let Err(e) = easy.dns_servers(&dns_string) {
            log::warn!("DNS servers could not be configured: {}", e);
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Proxy(pub(crate) http::Uri);

impl SetOpt for Proxy {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.proxy(&format!("{}", self.0))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SslCiphers(pub(crate) Vec<String>);

impl FromIterator<String> for SslCiphers {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        SslCiphers(Vec::from_iter(iter))
    }
}

impl SetOpt for SslCiphers {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.ssl_cipher_list(&self.0.join(":"))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AllowUnsafeSsl(pub(crate) bool);

impl SetOpt for AllowUnsafeSsl {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.ssl_verify_peer(!self.0)?;
        easy.ssl_verify_host(!self.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DisableConnectionCache(pub(crate) bool);

impl SetOpt for DisableConnectionCache {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.fresh_connect(self.0)?;
        easy.forbid_reuse(self.0)
    }
}
