use std::{
    error::Error,
    fmt,
    str,
    time::{Duration, SystemTime},
};

/// An error which can occur when attempting to parse a cookie string.
#[derive(Debug)]
pub struct ParseError(());

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid cookie string syntax")
    }
}

impl Error for ParseError {}

/// Builder for a [`Cookie`].
///
/// ```rust
/// use isahc::cookies::Cookie;
/// use std::time::{Duration, SystemTime};
///
/// let cookie: Cookie = Cookie::builder("name", "value") // or CookieBuilder::new("name", "value")
///     .domain("example.com")
///     .path("/")
///     .secure(true)
///     .expiration(SystemTime::now() + Duration::from_secs(30 * 60))
///     .build()
///     .unwrap();
/// ```
#[derive(Clone, Debug)]
#[must_use = "builders have no effect if unused"]
pub struct CookieBuilder {
    /// The name of the cookie.
    name: String,

    /// The cookie value.
    value: String,

    /// The domain the cookie belongs to.
    domain: Option<String>,

    /// A path prefix that this cookie belongs to.
    path: Option<String>,

    /// True if the cookie is marked as secure (limited in scope to HTTPS).
    secure: Option<bool>,

    /// Time when this cookie expires. If not present, then this is a session
    /// cookie that expires when the current client session ends.
    expiration: Option<SystemTime>,
}

impl CookieBuilder {
    /// Create a new cookie builder with a given name and value.
    #[allow(unused)]
    pub fn new<N, V>(name: N, value: V) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        Self {
            name: name.into(),
            value: value.into(),
            domain: None,
            path: None,
            secure: None,
            expiration: None,
        }
    }

    /// Sets the domain the cookie belongs to.
    pub fn domain<S>(mut self, domain: S) -> Self
    where
        S: Into<String>,
    {
        self.domain = Some(domain.into());
        self
    }

    /// Sets the path prefix that this cookie belongs to.
    pub fn path<S>(mut self, path: S) -> Self
    where
        S: Into<String>,
    {
        self.path = Some(path.into());
        self
    }

    /// True if the cookie is marked as secure (limited in scope to HTTPS).
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = Some(secure);
        self
    }

    /// Time when this cookie expires. If not present, then this is a session
    /// cookie that expires when the current client session ends.
    pub fn expiration<T>(mut self, expiration: T) -> Self
    where
        T: Into<SystemTime>,
    {
        self.expiration = Some(expiration.into());
        self
    }

    /// Builds the cookie.
    ///
    /// Returns an error if either the name or value given contains illegal
    /// characters. In practice, only a subset of US-ASCII characters are
    /// allowed in cookies for maximum compatibility with most web servers.
    pub fn build(self) -> Result<Cookie, ParseError> {
        let Self {
            name,
            value,
            domain,
            path,
            secure,
            expiration,
        } = self;

        let mut cookie = Cookie::new(name, value)?;
        cookie.domain = domain;
        cookie.path = path;
        cookie.expiration = expiration;

        if let Some(secure) = secure {
            cookie.secure = secure;
        }

        Ok(cookie)
    }
}

/// Information stored about an HTTP cookie.
///
/// # Comparison operators
///
/// You can use the equals operator to compare the value of a cookie with a string directly for convenience. In other words, this:
///
/// ```ignore
/// assert_eq!(cookie.value(), "foo");
/// ```
///
/// is equivalent to this:
///
/// ```ignore
/// assert_eq!(cookie, "foo");
/// ```
#[derive(Clone, Debug)]
pub struct Cookie {
    /// The name of the cookie.
    name: String,

    /// The cookie value.
    value: String,

    /// The domain the cookie belongs to.
    domain: Option<String>,

    /// A path prefix that this cookie belongs to.
    path: Option<String>,

    /// True if the cookie is marked as secure (limited in scope to HTTPS).
    secure: bool,

    /// Time when this cookie expires. If not present, then this is a session
    /// cookie that expires when the current client session ends.
    expiration: Option<SystemTime>,
}

impl Cookie {
    /// Create a new cookie with a given name and value.
    ///
    /// Returns an error if either the name or value given contains illegal
    /// characters. In practice, only a subset of US-ASCII characters are
    /// allowed in cookies for maximum compatibility with most web servers.
    #[allow(unused)]
    fn new<N, V>(name: N, value: V) -> Result<Self, ParseError>
    where
        N: Into<String>,
        V: Into<String>,
    {
        let name = name.into();
        let value = value.into();

        // Validate the characters of the name and value.
        if is_valid_token(name.as_bytes()) && is_valid_cookie_value(value.as_bytes()) {
            Ok(Self {
                name,
                value,
                domain: None,
                path: None,
                secure: false,
                expiration: None,
            })
        } else {
            Err(ParseError(()))
        }
    }

    /// Create a new cookie builder with a given name and value.
    /// See [`CookieBuilder::new`] for an example.
    #[allow(unused)]
    pub fn builder<N, V>(name: N, value: V) -> CookieBuilder
    where
        N: Into<String>,
        V: Into<String>,
    {
        CookieBuilder::new(name, value)
    }

    /// Parse a cookie from a cookie string, as defined in [RFC 6265, section
    /// 4.2.1](https://tools.ietf.org/html/rfc6265#section-4.2.1). This can be
    /// used to parse `Set-Cookie` header values, but not `Cookie` header
    /// values, which follow a slightly different syntax.
    ///
    /// If the given value is not a valid cookie string, an error is returned.
    /// Note that unknown attributes do not cause a parsing error, and are
    /// simply ignored (as per [RFC 6265, section
    /// 4.1.2](https://tools.ietf.org/html/rfc6265#section-4.1.2)).
    pub(crate) fn parse<T>(header: T) -> Result<Self, ParseError>
    where
        T: AsRef<[u8]>,
    {
        Self::parse_impl(header.as_ref())
    }

    /// Get the name of the cookie.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the value of the cookie.
    #[inline]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Get the domain of the cookie, if specified.
    #[inline]
    pub(crate) fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Get the path of the cookie, if specified.
    #[inline]
    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Get whether this cookie was marked as being secure only. If `true`, this
    /// cookie will only be sent to the server for HTTPS requests.
    #[inline]
    pub(crate) fn is_secure(&self) -> bool {
        self.secure
    }

    /// Get whether this cookie should be persisted across sessions.
    #[inline]
    #[allow(unused)]
    pub(crate) fn is_persistent(&self) -> bool {
        self.expiration.is_some()
    }

    /// Check if the cookie has expired.
    pub(crate) fn is_expired(&self) -> bool {
        if let Some(time) = self.expiration.as_ref() {
            *time < SystemTime::now()
        } else {
            false
        }
    }

    fn parse_impl(header: &[u8]) -> Result<Self, ParseError> {
        let mut attributes = trim_left_ascii(header)
            .split(|&byte| byte == b';')
            .map(trim_left_ascii);

        let first_pair = split_at_first(attributes.next().ok_or(ParseError(()))?, &b'=')
            .ok_or(ParseError(()))?;

        let cookie_name = parse_token(first_pair.0)?.into();
        let cookie_value = parse_cookie_value(first_pair.1)?.into();
        let mut cookie_domain = None;
        let mut cookie_path = None;
        let mut cookie_secure = false;
        let mut cookie_expiration = None;

        // Look for known attribute names and parse them. Note that there are
        // multiple attributes in the spec that we don't parse right now because we
        // do not care about them, including HttpOnly and SameSite.
        for attribute in attributes {
            if let Some((name, value)) = split_at_first(attribute, &b'=') {
                if name.eq_ignore_ascii_case(b"Expires") {
                    if cookie_expiration.is_none() {
                        if let Ok(value) = str::from_utf8(value) {
                            if let Ok(time) = httpdate::parse_http_date(value) {
                                cookie_expiration = Some(time);
                            }
                        }
                    }
                } else if name.eq_ignore_ascii_case(b"Domain") {
                    if let Ok(value) = str::from_utf8(value) {
                        cookie_domain = Some(value.trim_start_matches('.').to_lowercase());
                    }
                } else if name.eq_ignore_ascii_case(b"Max-Age") {
                    if let Ok(value) = str::from_utf8(value) {
                        if let Ok(seconds) = value.parse() {
                            cookie_expiration =
                                Some(SystemTime::now() + Duration::from_secs(seconds));
                        }
                    }
                } else if name.eq_ignore_ascii_case(b"Path") {
                    if let Ok(value) = str::from_utf8(value) {
                        cookie_path = Some(value.to_owned());
                    }
                }
            } else if attribute.eq_ignore_ascii_case(b"Secure") {
                cookie_secure = true;
            }
        }

        Ok(Self {
            name: cookie_name,
            value: cookie_value,
            secure: cookie_secure,
            expiration: cookie_expiration,
            domain: cookie_domain,
            path: cookie_path,
        })
    }
}

impl PartialEq<&str> for Cookie {
    fn eq(&self, other: &&str) -> bool {
        self.value.as_str() == *other
    }
}

impl PartialEq<String> for Cookie {
    fn eq(&self, other: &String) -> bool {
        self.value == *other
    }
}

// Maybe one day implement FromStr publicly.
// impl FromStr for Cookie {
//     type Err = ParseError;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Self::parse(s)
//     }
// }

// https://tools.ietf.org/html/rfc6265#section-4.1.1
#[allow(unsafe_code)]
fn parse_cookie_value(mut bytes: &[u8]) -> Result<&str, ParseError> {
    // Strip quotes, but only if in a legal pair.
    if bytes.starts_with(b"\"") && bytes.ends_with(b"\"") {
        bytes = &bytes[1..bytes.len() - 1];
    }

    // Validate the bytes are all legal cookie octets.
    if !is_valid_cookie_value(bytes) {
        return Err(ParseError(()));
    }

    // Safety: We know that the given bytes are valid US-ASCII at this point, so
    // therefore it is also valid UTF-8.
    Ok(unsafe { str::from_utf8_unchecked(bytes) })
}

// https://tools.ietf.org/html/rfc6265#section-4.1.1
fn is_valid_cookie_value(bytes: &[u8]) -> bool {
    bytes.iter().all(|&byte| match byte {
        0x21 | 0x23..=0x2B | 0x2D..=0x3A | 0x3C..=0x5B | 0x5D..=0x7E => true,
        _ => false,
    })
}

// https://tools.ietf.org/html/rfc2616#section-2.2
#[allow(unsafe_code)]
fn parse_token(bytes: &[u8]) -> Result<&str, ParseError> {
    if is_valid_token(bytes) {
        // Safety: We know that the given bytes are valid US-ASCII at this
        // point, so therefore it is also valid UTF-8.
        Ok(unsafe { str::from_utf8_unchecked(bytes) })
    } else {
        Err(ParseError(()))
    }
}

// https://tools.ietf.org/html/rfc2616#section-2.2
fn is_valid_token(bytes: &[u8]) -> bool {
    const SEPARATORS: &[u8] = b"()<>@,;:\\\"/[]?={} \t";

    bytes
        .iter()
        .all(|byte| byte.is_ascii() && !byte.is_ascii_control() && !SEPARATORS.contains(byte))
}

fn trim_left_ascii(mut ascii: &[u8]) -> &[u8] {
    while ascii.first() == Some(&b' ') {
        ascii = &ascii[1..];
    }

    ascii
}

fn split_at_first<'a, T: PartialEq>(slice: &'a [T], separator: &T) -> Option<(&'a [T], &'a [T])> {
    for (i, value) in slice.iter().enumerate() {
        if value == separator {
            return Some((&slice[..i], &slice[i + 1..]));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn system_time_timestamp(time: &SystemTime) -> u64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test_case("foo")]
    #[test_case("foo;=bar")]
    #[test_case("bad_name@?=bar")]
    #[test_case("bad_value_comma=bar,")]
    #[test_case("bad_value_space= bar")]
    fn parse_invalid(s: &str) {
        assert!(Cookie::parse(s).is_err());
    }

    #[test_case("foo=bar")]
    #[test_case(r#"foo="bar""#)]
    fn parse_simple(s: &str) {
        let cookie = Cookie::parse(s).unwrap();

        assert_eq!(cookie.name(), "foo");
        assert_eq!(cookie.value(), "bar");
        assert_eq!(cookie.path(), None);
        assert!(!cookie.is_secure());
        assert!(!cookie.is_persistent());
    }

    #[test]
    fn parse_persistent() {
        let cookie = Cookie::parse("foo=bar; max-age=86400").unwrap();

        assert_eq!(cookie.name(), "foo");
        assert_eq!(cookie.value(), "bar");
        assert_eq!(cookie.path(), None);
        assert!(!cookie.is_secure());
        assert!(cookie.is_persistent());
    }

    #[test]
    fn parse_set_cookie_header_expires() {
        let cookie = Cookie::parse(
            "foo=bar; path=/sub;Secure; DOMAIN=baz.com;expires=Wed, 21 Oct 2015 07:28:00 GMT",
        )
        .unwrap();

        assert_eq!(cookie.name(), "foo");
        assert_eq!(cookie.value(), "bar");
        assert_eq!(cookie.path(), Some("/sub"));
        assert_eq!(cookie.domain.as_deref(), Some("baz.com"));
        assert!(cookie.is_secure());
        assert!(cookie.is_expired());
        assert_eq!(
            cookie.expiration.as_ref().map(system_time_timestamp),
            Some(1_445_412_480)
        );
    }

    #[test]
    fn parse_set_cookie_header_max_age() {
        let cookie =
            Cookie::parse("foo=bar; path=/sub;Secure; DOMAIN=baz.com; max-age=60").unwrap();

        assert_eq!(cookie.name(), "foo");
        assert_eq!(cookie.value(), "bar");
        assert_eq!(cookie.path(), Some("/sub"));
        assert_eq!(cookie.domain.as_deref(), Some("baz.com"));
        assert!(cookie.is_secure());
        assert!(!cookie.is_expired());
        assert!(
            cookie
                .expiration
                .unwrap()
                .duration_since(SystemTime::now())
                .unwrap()
                <= Duration::from_secs(60)
        );
    }

    #[test]
    fn create_cookie() {
        let exp = SystemTime::now();

        let cookie = Cookie::builder("foo", "bar")
            .domain("baz.com")
            .path("/sub")
            .secure(true)
            .expiration(exp)
            .build()
            .unwrap();

        assert_eq!(cookie.name(), "foo");
        assert_eq!(cookie.value(), "bar");
        assert_eq!(cookie.path(), Some("/sub"));
        assert_eq!(cookie.domain.as_deref(), Some("baz.com"));
        assert!(cookie.is_secure());
        assert_eq!(cookie.expiration, Some(exp));
    }
}
