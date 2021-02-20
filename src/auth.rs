//! Types for working with HTTP authentication methods.

use crate::config::{proxy::Proxy, request::SetOpt};
use std::{
    fmt,
    ops::{BitOr, BitOrAssign},
};

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

/// Specifies one or more HTTP authentication schemes to use.
#[derive(Clone, Debug)]
pub struct Authentication(u8);

impl Default for Authentication {
    fn default() -> Self {
        Self::none()
    }
}

impl Authentication {
    /// Disable all authentication schemes. This is the default.
    pub const fn none() -> Self {
        Authentication(0)
    }

    /// Enable all available authentication schemes.
    pub const fn all() -> Self {
        #[allow(unused_mut)]
        let mut all = Self::basic().0 | Self::digest().0;

        #[cfg(feature = "spnego")]
        {
            all |= Self::negotiate().0;
        }

        Authentication(all)
    }

    /// HTTP Basic authentication.
    ///
    /// This authentication scheme sends the user name and password over the
    /// network in plain text. Avoid using this scheme without TLS as the
    /// credentials can be easily captured otherwise.
    pub const fn basic() -> Self {
        Authentication(0b0001)
    }

    /// HTTP Digest authentication.
    ///
    /// Digest authentication is defined in RFC 2617 and is a more secure way to
    /// do authentication over public networks than the regular old-fashioned
    /// Basic method.
    pub const fn digest() -> Self {
        Authentication(0b0010)
    }

    /// HTTP Negotiate (SPNEGO) authentication.
    ///
    /// Negotiate authentication is defined in RFC 4559 and is the most secure
    /// way to perform authentication over HTTP. Specifying [`Credentials`] is
    /// not necessary as credentials are provided by platform authentication
    /// means.
    ///
    /// You need to build libcurl with a suitable GSS-API library or SSPI on
    /// Windows for this to work. This is automatic when binding to curl
    /// statically, otherwise it depends on how your system curl is configured.
    ///
    /// # Availability
    ///
    /// This method is only available when the [`spnego`](../index.html#spnego)
    /// feature is enabled.
    #[cfg(feature = "spnego")]
    pub const fn negotiate() -> Self {
        Authentication(0b0100)
    }

    const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    fn as_auth(&self) -> curl::easy::Auth {
        let mut auth = curl::easy::Auth::new();

        if self.contains(Authentication::basic()) {
            auth.basic(true);
        }

        if self.contains(Authentication::digest()) {
            auth.digest(true);
        }

        #[cfg(feature = "spnego")]
        {
            if self.contains(Authentication::negotiate()) {
                auth.gssnegotiate(true);
            }
        }

        auth
    }
}

impl BitOr for Authentication {
    type Output = Self;

    fn bitor(mut self, other: Self) -> Self {
        self |= other;
        self
    }
}

impl BitOrAssign for Authentication {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl SetOpt for Authentication {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        #[cfg(feature = "spnego")]
        {
            if self.contains(Authentication::negotiate()) {
                // Ensure auth engine is enabled, even though credentials do not
                // need to be specified.
                easy.username("")?;
                easy.password("")?;
            }
        }

        easy.http_auth(&self.as_auth())
    }
}

impl SetOpt for Proxy<Authentication> {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        #[cfg(feature = "spnego")]
        {
            if self.0.contains(Authentication::negotiate()) {
                // Ensure auth engine is enabled, even though credentials do not
                // need to be specified.
                easy.proxy_username("")?;
                easy.proxy_password("")?;
            }
        }

        easy.proxy_auth(&self.0.as_auth())
    }
}

#[cfg(test)]
mod tests {
    use super::Authentication;

    #[test]
    fn auth_default() {
        let auth = Authentication::default();

        assert!(!auth.contains(Authentication::basic()));
        assert!(!auth.contains(Authentication::digest()));
    }

    #[test]
    fn auth_all() {
        let auth = Authentication::all();

        assert!(auth.contains(Authentication::basic()));
        assert!(auth.contains(Authentication::digest()));
    }

    #[test]
    fn auth_single() {
        let auth = Authentication::basic();

        assert!(auth.contains(Authentication::basic()));
        assert!(!auth.contains(Authentication::digest()));

        let auth = Authentication::digest();

        assert!(!auth.contains(Authentication::basic()));
        assert!(auth.contains(Authentication::digest()));
    }
}
