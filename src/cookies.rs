//! Cookie state management.
//!
//! This module provides a cookie jar implementation conforming to RFC 6265.

use http::header;
use http::Uri;
use middleware::Middleware;
use std::collections::HashMap;
use std::sync::RwLock;
use super::{Request, Response};

type Cookie = ::cookie::Cookie<'static>;

/// Provides automatic cookie session management using an in-memory cookie store.
#[derive(Default)]
pub struct CookieJar {
    /// A map of cookies indexed by a string of the format `{domain}.{path}.{name}`.
    cookies: RwLock<HashMap<String, Cookie>>,
}

impl Middleware for CookieJar {
    fn before(&self, mut request: Request) -> Request {
        let jar = self.cookies.read().unwrap();

        let values: Vec<String> = jar.values()
            .filter(|cookie| cookie_matches(cookie, request.uri()))
            .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
            .collect();

        if !values.is_empty() {
            request.headers_mut().insert(header::COOKIE, values.join("; ").parse().unwrap());
        }

        request
    }

    /// Extracts cookies set via the Set-Cookie header.
    fn after(&self, response: Response) -> Response {
        if response.headers().contains_key(header::SET_COOKIE) {
            let mut jar = self.cookies.write().unwrap();

            for header in response.headers().get_all(header::SET_COOKIE) {
                match header.to_str() {
                    Ok(header) => match Cookie::parse(header) {
                        Ok(cookie) => {
                            let cookie = cookie.into_owned();
                            let key = cookie_key(&cookie);
                            jar.insert(key, cookie);
                        },
                        Err(e) => warn!("could not parse Set-Cookie header: {}", e),
                    },
                    Err(_) => warn!("invalid encoding in Set-Cookie header"),
                }
            }
        }

        response
    }
}

fn cookie_key(cookie: &Cookie) -> String {
    format!("{}.{}.{}", cookie.domain().unwrap_or(""), cookie.path_raw().unwrap_or(""), cookie.name())
}

fn cookie_matches(cookie: &Cookie, uri: &Uri) -> bool {
    // Only match secure cookies when on HTTPS.
    if cookie.secure().unwrap_or(false) {
        if uri.scheme_part() != Some(&::http::uri::Scheme::HTTPS) {
            return false;
        }
    }

    // Cookie is restricted by domain.
    if let Some(domain) = cookie.domain() {
        if !uri.host().unwrap().eq_ignore_ascii_case(domain) {
            // TODO
            let domain = "".trim_start_matches(".");
            return false;
        }
    }

    if let Some(path) = cookie.path() {
        if uri.path().len() > 0 {
            if path != uri.path() {
                // TODO
                return false;
            }
        }
    }

    true
}
