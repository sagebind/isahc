//! Cookie state management.
//!
//! This module provides a cookie jar implementation conforming to RFC 6265.
//!
//! Everything in this module requires the `cookies` feature to be enabled.

use super::Cookie;
use http::Uri;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

type CookieJar2Obj = Box<dyn CookieJar2 + Send + Sync + 'static>;

pub trait CookieJar2 {
    fn get(&self, uri: &Uri) -> Option<Cookie>;

    fn insert(&self, cookies: &[Cookie]);
}

/// Provides automatic cookie session management using an in-memory cookie
/// store.
///
/// Cookie jars are designed to be shareable across many concurrent requests, so
/// cloning the jar simply returns a new reference to the jar instead of doing a
/// deep clone.
#[derive(Clone, Debug, Default)]
pub struct CookieJar {
    /// A map of cookies indexed by a string of the format
    /// `{domain}.{path}.{name}`.
    cookies: Arc<RwLock<HashMap<String, Cookie>>>,
}

impl CookieJar {
    /// Add all the cookies in the given iterator to the cookie jar.
    pub fn add(&self, cookies: impl Iterator<Item = Cookie>) {
        let mut jar = self.cookies.write().unwrap();

        for cookie in cookies {
            jar.insert(cookie.key(), cookie);
        }

        // Clear expired cookies while we have a write lock.
        jar.retain(|_, cookie| !cookie.is_expired());
    }

    /// Remove all cookies from this cookie jar.
    pub fn clear(&self) {
        self.cookies.write().unwrap().clear();
    }

    pub(crate) fn get_cookies(&self, uri: &Uri) -> Option<String> {
        let jar = self.cookies.read().unwrap();

        let mut values: Vec<String> = jar
            .values()
            .filter(|cookie| cookie.matches(uri))
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect();

        if values.is_empty() {
            None
        } else {
            // Cookies should be returned in lexical order.
            values.sort();

            Some(values.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expire_a_cookie() {
        let uri: Uri = "https://example.com/foo".parse().unwrap();
        let jar = CookieJar::default();

        jar.add(Cookie::parse("foo=bar", &uri).into_iter());

        assert_eq!(jar.get_cookies(&uri).unwrap(), "foo=bar");

        jar.add(Cookie::parse("foo=; expires=Wed, 21 Oct 2015 07:28:00 GMT", &uri).into_iter());

        assert_eq!(jar.get_cookies(&uri), None);
    }
}
