use super::psl;
use chrono::{
    prelude::*,
    Duration,
};
use http::Uri;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Information stored about an HTTP cookie.
#[derive(Debug)]
pub struct Cookie {
    /// The name of the cookie.
    pub(crate) name: String,
    /// The cookie value.
    pub(crate) value: String,
    /// The domain the cookie belongs to.
    pub(crate) domain: String,
    /// A path prefix that this cookie belongs to.
    pub(crate) path: String,
    /// True if the cookie is marked as secure (limited in scope to HTTPS).
    pub(crate) secure: bool,
    /// True if the cookie is a host-only cookie (i.e. the request's host must
    /// exactly match the domain of the cookie).
    pub(crate) host_only: bool,
    /// Time when this cookie expires. If not present, then this is a session
    /// cookie that expires when the current client session ends.
    pub(crate) expiration: Option<DateTime<Utc>>,
}

impl Cookie {
    /// Parse a cookie from a Set-Cookie header value, within the context of the
    /// given URI.
    pub(crate) fn parse(header: &str, uri: &Uri) -> Option<Self> {
        let mut attributes = header
            .split(';')
            .map(str::trim)
            .map(|item| item.splitn(2, '=').map(str::trim));

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
                cookie_domain = value
                    .map(|s| s.trim_start_matches('.'))
                    .map(str::to_lowercase);
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

        // Perform some validations on the domain.
        if let Some(domain) = cookie_domain.as_ref() {
            // The given domain must domain-match the origin.
            // https://tools.ietf.org/html/rfc6265#section-5.3.6
            if !Cookie::domain_matches(uri.host()?, domain) {
                tracing::warn!(
                    "cookie '{}' dropped, domain '{}' not allowed to set cookies for '{}'",
                    cookie_name,
                    uri.host()?,
                    domain
                );
                return None;
            }

            // Drop cookies for top-level domains.
            if !domain.contains('.') {
                tracing::warn!(
                    "cookie '{}' dropped, setting cookies for domain '{}' is not allowed",
                    cookie_name,
                    domain
                );
                return None;
            }

            // Check the PSL for bad domain suffixes if available.
            // https://tools.ietf.org/html/rfc6265#section-5.3.5
            #[cfg(feature = "psl")]
            {
                if psl::is_public_suffix(domain) {
                    tracing::warn!(
                        "cookie '{}' dropped, setting cookies for domain '{}' is not allowed",
                        cookie_name,
                        domain
                    );
                    return None;
                }
            }
        }

        Some(Self {
            name: cookie_name,
            value: cookie_value,
            secure: cookie_secure,
            expiration: cookie_expiration,
            host_only: cookie_domain.is_none(),
            domain: cookie_domain.or_else(|| uri.host().map(ToOwned::to_owned))?,
            path: cookie_path.unwrap_or_else(|| Cookie::default_path(uri).to_owned()),
        })
    }

    pub(crate) fn is_expired(&self) -> bool {
        match self.expiration {
            Some(time) => time < Utc::now(),
            None => false,
        }
    }

    pub(crate) fn key(&self) -> String {
        format!("{}.{}.{}", self.domain, self.path, self.name)
    }

    // http://tools.ietf.org/html/rfc6265#section-5.4
    pub(crate) fn matches(&self, uri: &Uri) -> bool {
        if self.secure && uri.scheme() != Some(&::http::uri::Scheme::HTTPS) {
            return false;
        }

        let request_host = uri.host().unwrap_or("");

        if self.host_only {
            if !self.domain.eq_ignore_ascii_case(request_host) {
                return false;
            }
        } else if !Cookie::domain_matches(request_host, &self.domain) {
            return false;
        }

        if !Cookie::path_matches(uri.path(), &self.path) {
            return false;
        }

        if self.is_expired() {
            return false;
        }

        true
    }

    // http://tools.ietf.org/html/rfc6265#section-5.1.3
    pub(crate) fn domain_matches(string: &str, domain_string: &str) -> bool {
        if domain_string.eq_ignore_ascii_case(string) {
            return true;
        }

        let string = &string.to_lowercase();
        let domain_string = &domain_string.to_lowercase();

        string.ends_with(domain_string)
            && string.as_bytes()[string.len() - domain_string.len() - 1] == b'.'
            && string.parse::<Ipv4Addr>().is_err()
            && string.parse::<Ipv6Addr>().is_err()
    }

    // http://tools.ietf.org/html/rfc6265#section-5.1.4
    pub(crate) fn path_matches(request_path: &str, cookie_path: &str) -> bool {
        if request_path == cookie_path {
            return true;
        }

        if request_path.starts_with(cookie_path)
            && (cookie_path.ends_with('/') || request_path[cookie_path.len()..].starts_with('/'))
        {
            return true;
        }

        false
    }

    // http://tools.ietf.org/html/rfc6265#section-5.1.4
    fn default_path(uri: &Uri) -> &str {
        // Step 2
        if !uri.path().starts_with('/') {
            return "/";
        }

        // Step 3
        let rightmost_slash_idx = uri.path().rfind('/').unwrap();
        if rightmost_slash_idx == 0 {
            // There's only one slash; it's the first character.
            return "/";
        }

        // Step 4
        &uri.path()[..rightmost_slash_idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_cookie(header: &str, uri: &str) -> Option<Cookie> {
        Cookie::parse(header, &uri.parse().unwrap())
    }

    #[test]
    fn parse_set_cookie_header() {
        let cookie = parse_cookie(
            "foo=bar; path=/sub;Secure ; expires =Wed, 21 Oct 2015 07:28:00 GMT",
            "https://baz.com",
        )
        .unwrap();

        assert_eq!(cookie.name, "foo");
        assert_eq!(cookie.value, "bar");
        assert_eq!(cookie.path, "/sub");
        assert_eq!(cookie.domain, "baz.com");
        assert!(cookie.secure);
        assert!(cookie.host_only);
        assert_eq!(
            cookie.expiration.as_ref().map(|t| t.timestamp()),
            Some(1_445_412_480)
        );
    }

    #[test]
    fn cookie_domain_not_allowed() {
        assert!(parse_cookie("foo=bar", "https://bar.baz.com").is_some());
        assert!(parse_cookie("foo=bar; domain=bar.baz.com", "https://bar.baz.com").is_some());
        assert!(parse_cookie("foo=bar; domain=baz.com", "https://bar.baz.com").is_some());
        assert!(parse_cookie("foo=bar; domain=www.bar.baz.com", "https://bar.baz.com").is_none());

        // TLDs are not allowed.
        assert!(parse_cookie("foo=bar; domain=com", "https://bar.baz.com").is_none());
        assert!(parse_cookie("foo=bar; domain=.com", "https://bar.baz.com").is_none());

        // If the public suffix list is enabled, also exercise that validation.
        if cfg!(feature = "psl") {
            // wi.us is a public suffix
            assert!(parse_cookie("foo=bar; domain=wi.us", "https://www.state.wi.us").is_none());
        }
    }

    #[test]
    fn domain_matches() {
        for case in &[
            ("127.0.0.1", "127.0.0.1", true),
            (".127.0.0.1", "127.0.0.1", true),
            ("bar.com", "bar.com", true),
            ("baz.com", "bar.com", false),
            ("baz.bar.com", "bar.com", true),
            ("www.baz.com", "baz.com", true),
            ("baz.bar.com", "com", true),
        ] {
            assert_eq!(Cookie::domain_matches(case.0, case.1), case.2);
        }
    }

    #[test]
    fn path_matches() {
        for case in &[
            ("/foo", "/foo", true),
            ("/Foo", "/foo", false),
            ("/fo", "/foo", false),
            ("/foo/bar", "/foo", true),
            ("/foo/bar/baz", "/foo", true),
            ("/foo/bar//baz", "/foo", true),
            ("/foobar", "/foo", false),
            ("/foo", "/foo/bar", false),
            ("/foobar", "/foo/bar", false),
            ("/foo/bar", "/foo/bar", true),
            ("/foo/bar/", "/foo/bar", true),
            ("/foo/bar/baz", "/foo/bar", true),
            ("/foo/bar", "/foo/bar/", false),
            ("/foo/bar/", "/foo/bar/", true),
            ("/foo/bar/baz", "/foo/bar/", true),
        ] {
            assert_eq!(Cookie::path_matches(case.0, case.1), case.2);
        }
    }
}
