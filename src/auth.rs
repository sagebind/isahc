//! Types for working with HTTP authentication methods.

use crate::config::SetOpt;
use std::fmt;

enum Authentication {
    /// Username and password authentication, used by most schemes.
    Credentials(Credentials),

    /// Bearer token authentication scheme, typically used for OAuth 2.0.
    Bearer(String),
}

/// Credentials consisting of a username and a secret (password) that can be
/// used to establish user identity.
#[derive(Clone)]
pub struct Credentials {
    username: String,
    password: String,
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

impl SetOpt for Credentials {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        easy.username(&self.username)?;
        easy.password(&self.password)
    }
}
