use chrono::{prelude::*, Duration};
use std::{
    error::Error,
    fmt,
    str::{self, FromStr},
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

/// Information stored about an HTTP cookie.
#[derive(Clone, Debug)]
pub struct Cookie {
    /// The name of the cookie.
    name: String,

    /// The cookie value.
    value: String,

    /// The domain the cookie belongs to.
    pub(crate) domain: Option<String>,

    /// A path prefix that this cookie belongs to.
    pub(crate) path: Option<String>,

    /// True if the cookie is marked as secure (limited in scope to HTTPS).
    secure: bool,

    /// Time when this cookie expires. If not present, then this is a session
    /// cookie that expires when the current client session ends.
    expiration: Option<DateTime<Utc>>,
}

impl Cookie {
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

    /// Get whether this cookie was marked as being secure only.
    #[inline]
    pub(crate) fn is_secure(&self) -> bool {
        self.secure
    }

    pub(crate) fn is_expired(&self) -> bool {
        match self.expiration {
            Some(time) => time < Utc::now(),
            None => false,
        }
    }

    fn parse_impl(header: &[u8]) -> Result<Self, ParseError> {
        let mut attributes = trim_left_ascii(header)
            .split(|&byte| byte == b';')
            .map(trim_left_ascii);

        let first_pair =
            split_at_first(attributes.next().ok_or(ParseError(()))?, &b'=').ok_or(ParseError(()))?;

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
                            if let Ok(time) = DateTime::parse_from_rfc2822(value) {
                                cookie_expiration = Some(time.with_timezone(&Utc));
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
                            cookie_expiration = Some(Utc::now() + Duration::seconds(seconds));
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

impl FromStr for Cookie {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// https://tools.ietf.org/html/rfc6265#section-4.1.1
#[allow(unsafe_code)]
fn parse_cookie_value(mut bytes: &[u8]) -> Result<&str, ParseError> {
    // Strip quotes, but only if in a legal pair.
    if bytes.starts_with(b"\"") && bytes.ends_with(b"\"") {
        bytes = &bytes[1..bytes.len() - 2];
    }

    // Validate the bytes are all legal cookie octets.
    if !bytes.iter().copied().all(is_cookie_octet) {
        return Err(ParseError(()));
    }

    // Safety: We know that the given bytes are valid US-ASCII at this point, so
    // therefore it is also valid UTF-8.
    Ok(unsafe { str::from_utf8_unchecked(bytes) })
}

// https://tools.ietf.org/html/rfc2616#section-2.2
#[allow(unsafe_code)]
fn parse_token(bytes: &[u8]) -> Result<&str, ParseError> {
    const SEPARATORS: &[u8] = b"()<>@,;:\\\"/[]?={} \t";

    for byte in bytes {
        if !byte.is_ascii() || byte.is_ascii_control() || SEPARATORS.contains(byte) {
            return Err(ParseError(()));
        }
    }

    // Safety: We know that the given bytes are valid US-ASCII at this point, so
    // therefore it is also valid UTF-8.
    Ok(unsafe { str::from_utf8_unchecked(bytes) })
}

fn is_cookie_octet(byte: u8) -> bool {
    match byte {
        0x21 | 0x23..=0x2B | 0x2D..=0x3A | 0x3C..=0x5B | 0x5D..=0x7E => true,
        _ => false,
    }
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

    #[test]
    fn parse_set_cookie_header() {
        let cookie =
            "foo=bar; path=/sub;Secure; DOMAIN=baz.com;expires=Wed, 21 Oct 2015 07:28:00 GMT"
                .parse::<Cookie>()
                .unwrap();

        assert_eq!(cookie.name, "foo");
        assert_eq!(cookie.value, "bar");
        assert_eq!(cookie.path.as_deref(), Some("/sub"));
        assert_eq!(cookie.domain.as_deref(), Some("baz.com"));
        assert!(cookie.secure);
        assert_eq!(
            cookie.expiration.as_ref().map(|t| t.timestamp()),
            Some(1_445_412_480)
        );
    }
}
