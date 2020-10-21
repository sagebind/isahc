use super::Cookie;
use http::Uri;
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    net::{Ipv4Addr, Ipv6Addr},
    sync::{Arc, RwLock},
};

/// Provides automatic cookie session management using an in-memory cookie
/// store.
///
/// Cookie jars are designed to be shareable across many concurrent requests, so
/// cloning the jar simply returns a new reference to the jar instead of doing a
/// deep clone.
///
/// This cookie jar implementation seeks to conform to the rules for client
/// state management as described in [RFC
/// 6265](https://tools.ietf.org/html/rfc6265).
///
/// # Domain isolation
///
/// Cookies are isolated from each other based on the domain and path they are
/// received from. As such, most methods require you to specify a URI, since
/// unrelated websites can have cookies with the same name without conflict.
#[derive(Clone, Debug, Default)]
pub struct CookieJar {
    cookies: Arc<RwLock<HashSet<CookieWithContext>>>,
}

impl CookieJar {
    /// Create a new, empty cookie jar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a cookie by name for the given URI.
    pub fn get_by_name(&self, uri: &Uri, cookie_name: &str) -> Option<Cookie> {
        self.cookies
            .read()
            .unwrap()
            .iter()
            .filter(|cookie| cookie.matches(uri))
            .filter(|cookie| cookie.cookie.name() == cookie_name)
            .map(|c| c.cookie.clone())
            .next()
    }

    /// Get a copy of all the cookies in the jar that match the given URI.
    ///
    /// The returned collection contains a copy of all the cookies matching the
    /// URI at the time this function was called. The collection is not a "live"
    /// view into the cookie jar; concurrent changes made to the jar (cookies
    /// inserted or removed) will not be reflected in the collection.
    pub fn get_for_uri(&self, uri: &Uri) -> impl IntoIterator<Item = Cookie> {
        let jar = self.cookies.read().unwrap();

        let mut cookies = jar.iter()
            .filter(|cookie| cookie.matches(uri))
            .map(|c| c.cookie.clone())
            .collect::<Vec<_>>();

        // Cookies should be returned in lexical order.
        cookies.sort_by(|a, b| a.name().cmp(b.name()));

        cookies
    }

    /// Remove all cookies from this cookie jar.
    pub fn clear(&self) {
        self.cookies.write().unwrap().clear();
    }

    /// Set a cookie for the given absolute request URI.
    ///
    /// Returns true if the cookie was set, or false if the cookie was rejected.
    pub(crate) fn set(&self, cookie: Cookie, request_uri: &Uri) -> bool {
        let request_host = if let Some(host) = request_uri.host() {
            host
        } else {
            tracing::warn!(
                "cookie '{}' dropped, no domain specified in request URI",
                cookie.name()
            );
            return false;
        };

        // Perform some validations on the domain.
        if let Some(domain) = cookie.domain() {
            // The given domain must domain-match the origin.
            // https://tools.ietf.org/html/rfc6265#section-5.3.6
            if !domain_matches(request_host, domain) {
                tracing::warn!(
                    "cookie '{}' dropped, domain '{}' not allowed to set cookies for '{}'",
                    cookie.name(),
                    request_host,
                    domain
                );
                return false;
            }

            // Drop cookies for top-level domains.
            if !domain.contains('.') {
                tracing::warn!(
                    "cookie '{}' dropped, setting cookies for domain '{}' is not allowed",
                    cookie.name(),
                    domain
                );
                return false;
            }

            // Check the PSL for bad domain suffixes if available.
            // https://tools.ietf.org/html/rfc6265#section-5.3.5
            #[cfg(feature = "psl")]
            {
                if super::psl::is_public_suffix(domain) {
                    tracing::warn!(
                        "cookie '{}' dropped, setting cookies for domain '{}' is not allowed",
                        cookie.name(),
                        domain
                    );
                    return false;
                }
            }
        }

        let cookie_with_context = CookieWithContext {
            domain_value: cookie
                .domain()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| request_host.to_owned()),
            path_value: cookie
                .path()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| default_path(request_uri).to_owned()),
            cookie,
        };

        // Insert the cookie.
        let mut jar = self.cookies.write().unwrap();
        jar.replace(cookie_with_context);

        // Clear expired cookies while we have a write lock.
        jar.retain(|cookie| !cookie.cookie.is_expired());

        true
    }
}

/// Cookies with context is all the sweeter!
///
/// A persisted cookie including the context required to match the cookie
/// against outgoing requests. This type also implements `Eq` and `Hash` such
/// that cookies with the same domain, path, and name are considered the same,
/// as per RFC 6265 semantics.
#[derive(Debug)]
struct CookieWithContext {
    /// The domain-value of the cookie, as defined in RFC 6265. Will be derived
    /// from the request URI if the cookie did not specify one.
    domain_value: String,

    /// The path-value of the cookie, as defined in RFC 6265. Will be derived
    /// from the request URI if the cookie did not specify one.
    path_value: String,

    // The original cookie.
    cookie: Cookie,
}

impl CookieWithContext {
    /// True if the cookie is a host-only cookie (i.e. the request's host must
    /// exactly match the domain of the cookie).
    fn is_host_only(&self) -> bool {
        self.cookie.domain().is_none()
    }

    // http://tools.ietf.org/html/rfc6265#section-5.4
    fn matches(&self, uri: &Uri) -> bool {
        if self.cookie.is_secure() && uri.scheme() != Some(&::http::uri::Scheme::HTTPS) {
            return false;
        }

        let request_host = uri.host().unwrap_or("");

        if self.is_host_only() {
            if !self.domain_value.eq_ignore_ascii_case(request_host) {
                return false;
            }
        } else if !domain_matches(request_host, &self.domain_value) {
            return false;
        }

        if !path_matches(uri.path(), &self.path_value) {
            return false;
        }

        if self.cookie.is_expired() {
            return false;
        }

        true
    }
}

impl Hash for CookieWithContext {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.domain_value.hash(state);
        self.path_value.hash(state);
        self.cookie.name().hash(state);
    }
}

impl PartialEq for CookieWithContext {
    fn eq(&self, other: &Self) -> bool {
        self.domain_value == other.domain_value
            && self.path_value == other.path_value
            && self.cookie.name() == other.cookie.name()
    }
}

impl Eq for CookieWithContext {}

// http://tools.ietf.org/html/rfc6265#section-5.1.3
fn domain_matches(string: &str, domain_string: &str) -> bool {
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
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn cookie_domain_not_allowed() {
        let jar = CookieJar::default();

        assert!(jar.set(
            Cookie::parse("foo=bar").unwrap(),
            &"https://bar.baz.com".parse().unwrap()
        ));
        assert!(jar.set(
            Cookie::parse("foo=bar; domain=bar.baz.com").unwrap(),
            &"https://bar.baz.com".parse().unwrap()
        ));
        assert!(jar.set(
            Cookie::parse("foo=bar; domain=baz.com").unwrap(),
            &"https://bar.baz.com".parse().unwrap()
        ));
        assert!(!jar.set(
            Cookie::parse("foo=bar; domain=www.bar.baz.com").unwrap(),
            &"https://bar.baz.com".parse().unwrap()
        ));

        // TLDs are not allowed.
        assert!(!jar.set(
            Cookie::parse("foo=bar; domain=com").unwrap(),
            &"https://bar.baz.com".parse().unwrap()
        ));
        assert!(!jar.set(
            Cookie::parse("foo=bar; domain=.com").unwrap(),
            &"https://bar.baz.com".parse().unwrap()
        ));

        // If the public suffix list is enabled, also exercise that validation.
        if cfg!(feature = "psl") {
            // wi.us is a public suffix
            assert!(!jar.set(
                Cookie::parse("foo=bar; domain=wi.us").unwrap(),
                &"https://www.state.wi.us".parse().unwrap()
            ));
        }
    }

    #[test]
    fn expire_a_cookie() {
        let uri: Uri = "https://example.com/foo".parse().unwrap();
        let jar = CookieJar::default();

        jar.set(Cookie::parse("foo=bar").unwrap(), &uri);

        assert_eq!(jar.get_by_name(&uri, "foo").unwrap(), "bar");

        jar.set(
            Cookie::parse("foo=; expires=Wed, 21 Oct 2015 07:28:00 GMT").unwrap(),
            &uri,
        );

        assert!(jar.get_for_uri(&uri).into_iter().next().is_none());
    }

    #[test_case("127.0.0.1", "127.0.0.1", true)]
    #[test_case(".127.0.0.2", "127.0.0.2", true)]
    #[test_case("bar.com", "bar.com", true)]
    #[test_case("baz.com", "bar.com", false)]
    #[test_case("baz.bar.com", "bar.com", true)]
    #[test_case("www.baz.com", "baz.com", true)]
    #[test_case("baz.bar.com", "com", true)]
    fn test_domain_matches(string: &str, domain_string: &str, should_match: bool) {
        assert_eq!(domain_matches(string, domain_string), should_match);
    }

    #[test_case("/foo", "/foo", true)]
    #[test_case("/Bar", "/bar", false)]
    #[test_case("/fo", "/foo", false)]
    #[test_case("/foo/bar", "/foo", true)]
    #[test_case("/foo/bar/baz", "/foo", true)]
    #[test_case("/foo/bar//baz2", "/foo", true)]
    #[test_case("/foobar", "/foo", false)]
    #[test_case("/foo", "/foo/bar", false)]
    #[test_case("/foobar", "/foo/bar", false)]
    #[test_case("/foo/bar", "/foo/bar", true)]
    #[test_case("/foo/bar2/", "/foo/bar2", true)]
    #[test_case("/foo/bar/baz", "/foo/bar", true)]
    #[test_case("/foo/bar3", "/foo/bar3/", false)]
    #[test_case("/foo/bar4/", "/foo/bar4/", true)]
    #[test_case("/foo/bar/baz2", "/foo/bar/", true)]
    fn test_path_matches(request_path: &str, cookie_path: &str, should_match: bool) {
        assert_eq!(path_matches(request_path, cookie_path), should_match);
    }
}
