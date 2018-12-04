//! Cookie state management.
//!
//! This module provides a cookie jar implementation conforming to RFC 6265.

use chrono::Duration;
use chrono::prelude::*;
use http::header;
use http::Uri;
use middleware::Middleware;
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::RwLock;
use super::{Request, Response};

/// Information stored about an HTTP cookie.
pub struct Cookie {
    /// The name of the cookie.
    name: String,
    /// The cookie value.
    value: String,
    /// The domain the cookie belongs to.
    domain: String,
    /// A path prefix that this cookie belongs to.
    path: String,
    /// True if the cookie is marked as secure (limited in scope to HTTPS).
    secure: bool,
    /// True if the cookie is a host-only cookie (i.e. the request's host must exactly match the domain of the cookie).
    host_only: bool,
    /// Time when this cookie expires. If not present, then this is a session cookie that expires when the current
    /// client session ends.
    expiration: Option<DateTime<Utc>>,
}

impl Cookie {
    /// Parse a cookie from a Set-Cookie header value, within the context of the given URI.
    fn parse(header: &str, uri: &Uri) -> Option<Self> {
        let mut attributes = header.split(";")
            .map(str::trim)
            .map(|item| item
                .splitn(2, "=")
                .map(str::trim));

        let mut first_pair = attributes.next()?;

        let cookie_name = first_pair.next()?.into();
        let cookie_value = first_pair.next()?.into();
        let mut cookie_domain = None;
        let mut cookie_path = None;
        let mut cookie_secure = false;
        let mut cookie_expiration = None;

        // Look for known attribute names and parse them. Note that there are
        // multiple attributes in the spec that we don't parse right now because
        // we do not care about them, including HttpOnly and SameSite.
        for mut attribute in attributes {
            let name = attribute.next()?;
            let value = attribute.next();

            if name.eq_ignore_ascii_case("Expires") {
                if cookie_expiration.is_none() {
                    if let Some(value) = value {
                        if let Ok(time) = DateTime::parse_from_rfc2822(value) {
                            cookie_expiration = Some(time.with_timezone(&Utc));
                        }
                    }
                }
            } else if name.eq_ignore_ascii_case("Domain") {
                cookie_domain = value.map(ToOwned::to_owned);
            } else if name.eq_ignore_ascii_case("Max-Age") {
                if let Some(value) = value {
                    if let Ok(seconds) = value.parse() {
                        cookie_expiration = Some(Utc::now() + Duration::seconds(seconds));
                    }
                }
            } else if name.eq_ignore_ascii_case("Path") {
                cookie_path = value.map(ToOwned::to_owned);
            } else if name.eq_ignore_ascii_case("Secure") {
                cookie_secure = true;
            }
        }

        Some(Self {
            name: cookie_name,
            value: cookie_value,
            secure: cookie_secure,
            expiration: cookie_expiration,
            host_only: cookie_domain.is_none(),
            domain: cookie_domain.or_else(|| {
                uri.host().map(ToOwned::to_owned)
            })?,
            path: cookie_path.unwrap_or_else(|| {
                Cookie::default_path(uri).to_owned()
            }),
        })
    }

    fn is_expired(&self) -> bool {
        match self.expiration {
            Some(time) => time < Utc::now(),
            None => false,
        }
    }

    fn key(&self) -> String {
        format!("{}.{}.{}", self.domain, self.path, self.name)
    }

    // http://tools.ietf.org/html/rfc6265#section-5.4
    fn matches(&self, uri: &Uri) -> bool {
        if self.secure && uri.scheme_part() != Some(&::http::uri::Scheme::HTTPS) {
            return false;
        }

        if !self.matches_domain(uri.host().unwrap_or("")) {
            return false;
        }

        if !self.matches_path(uri.path()) {
            return false;
        }

        if self.is_expired() {
            return false;
        }

        true
    }

    // http://tools.ietf.org/html/rfc6265#section-5.1.3
    fn matches_domain(&self, domain: &str) -> bool {
        if self.domain.eq_ignore_ascii_case(domain) {
            return true;
        }

        if !self.host_only {
            let string = &domain.to_lowercase();
            let domain_string = &self.domain.to_lowercase();

            return string.ends_with(domain_string) &&
                string.as_bytes()[string.len() - domain_string.len() - 1] == b'.' &&
                string.parse::<Ipv4Addr>().is_err() &&
                string.parse::<Ipv6Addr>().is_err();
        }

        false
    }

    // http://tools.ietf.org/html/rfc6265#section-5.1.4
    fn matches_path(&self, path: &str) -> bool {
        if self.path == path {
            return true;
        }

        if path.starts_with(&self.path) {
            if self.path.ends_with("/") || path[self.path.len()..].starts_with("/") {
                return true;
            }
        }

        false
    }

    // http://tools.ietf.org/html/rfc6265#section-5.1.4
    fn default_path(uri: &Uri) -> &str {
        // Step 2
        if uri.path().chars().next() != Some('/') {
            return "/";
        }

        // Step 3
        let rightmost_slash_idx = uri.path().rfind("/").unwrap();
        if rightmost_slash_idx == 0 {
            // There's only one slash; it's the first character.
            return "/";
        }

        // Step 4
        &uri.path()[..rightmost_slash_idx]
    }
}

/// Provides automatic cookie session management using an in-memory cookie store.
#[derive(Default)]
pub struct CookieJar {
    /// A map of cookies indexed by a string of the format `{domain}.{path}.{name}`.
    cookies: RwLock<HashMap<String, Cookie>>,
}

impl CookieJar {
    /// Add all the cookies in the given iterator to the cookie jar.
    pub fn add(&self, cookies: impl Iterator<Item=Cookie>) {
        let mut jar = self.cookies.write().unwrap();

        for cookie in cookies {
            jar.insert(cookie.key(), cookie);
        }

        // Clear expired cookies while we have a write lock.
        jar.retain(|_, cookie| {
            !cookie.is_expired()
        });
    }
}

impl Middleware for CookieJar {
    fn filter_request(&self, mut request: Request) -> Request {
        let jar = self.cookies.read().unwrap();

        let mut values: Vec<String> = jar.values()
            .filter(|cookie| cookie.matches(request.uri()))
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect();

        // Cookies should be returned in lexical order.
        values.sort();

        if !values.is_empty() {
            request.headers_mut().insert(header::COOKIE, values.join("; ").parse().unwrap());
        }

        request
    }

    /// Extracts cookies set via the Set-Cookie header.
    fn filter_response(&self, response: Response) -> Response {
        if response.headers().contains_key(header::SET_COOKIE) {
            let cookies = response.headers()
                .get_all(header::SET_COOKIE)
                .into_iter()
                .filter_map(|header| {
                    match header.to_str() {
                        Ok(header) => match Cookie::parse(header, response.extensions().get().unwrap()) {
                            Some(cookie) => return Some(cookie),
                            _ => warn!("could not parse Set-Cookie header"),
                        },
                        _ => warn!("invalid encoding in Set-Cookie header"),
                    }
                    None
                });

            self.add(cookies);
        }

        response
    }
}
