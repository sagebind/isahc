//! Types for working with HTTP authentication methods.

use crate::config::{Proxy, SetOpt};
use std::fmt;

/// Credentials consisting of a username and a secret (password) that can be
/// used to establish user identity.
#[derive(Clone)]
pub struct Credentials {
    username: String,
    password: String,
}

impl Credentials {
    /// Create credentials from a username and password.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

impl SetOpt for Credentials {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.username(&self.username)?;
        easy.password(&self.password)
    }
}

impl SetOpt for Proxy<Credentials> {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.proxy_username(&self.0.username)?;
        easy.proxy_password(&self.0.password)
    }
}

// Implement our own debug since we don't want to print passwords even on
// accident.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"*****")
            .finish()
    }
}

/// Specifies one or more HTTP authentication methods to use.
#[derive(Clone, Debug)]
pub struct Authentication {
    inner: curl::easy::Auth,
    #[cfg(feature = "spnego")]
    negotiate: bool,
}

impl Default for Authentication {
    fn default() -> Self {
        Self::new()
    }
}

impl Authentication {
    /// Create a new empty set of authentication schemes.
    pub fn new() -> Self {
        Self {
            inner: curl::easy::Auth::new(),
            #[cfg(feature = "spnego")]
            negotiate: false,
        }
    }

    /// Enable all available authentication schemes.
    pub fn all() -> Self {
        #[allow(unused_mut)]
        let mut all = Self::new()
            .basic(true)
            .digest(true);

        #[cfg(feature = "spnego")]
        {
            all = all.negotiate(true);
        }

        all
    }

    /// HTTP Basic authentication.
    ///
    /// This authentication scheme sends the user name and password over the
    /// network in plain text. Avoid using this scheme without TLS as the
    /// credentials can be easily captured otherwise.
    pub fn basic(mut self, on: bool) -> Self {
        self.inner.basic(on);
        self
    }

    /// HTTP Digest authentication.
    ///
    /// Digest authentication is defined in RFC 2617 and is a more secure way to
    /// do authentication over public networks than the regular old-fashioned
    /// Basic method.
    pub fn digest(mut self, on: bool) -> Self {
        self.inner.digest(on);
        self
    }

    /// HTTP Negotiate (SPNEGO) authentication.
    ///
    /// Negotiate authentication is defined in RFC 4559 and is the most secure
    /// way to perform authentication over HTTP.
    ///
    /// You need to build libcurl with a suitable GSS-API library or SSPI on
    /// Windows for this to work. This is automatic when binding to curl
    /// statically, otherwise it depends on how your system curl is configured.
    #[cfg(feature = "spnego")]
    pub fn negotiate(mut self, on: bool) -> Self {
        self.negotiate = on;
        self.inner.gssnegotiate(on);
        self
    }
}

impl SetOpt for Authentication {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        #[cfg(feature = "spnego")]
        {
            if self.negotiate {
                easy.username(":")?;
            }
        }

        easy.http_auth(&self.inner)
    }
}

impl SetOpt for Proxy<Authentication> {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        #[cfg(feature = "spnego")]
        {
            if self.negotiate {
                easy.proxy_username(":")?;
            }
        }

        easy.proxy_auth(&self.0.inner)
    }
}
