use http::{StatusCode, Version};
use http::header::*;
use regex::bytes::Regex;

lazy_static! {
    static ref STATUS_LINE_REGEX: Regex = r#"^HTTP/(\d(?:\.\d)?) (\d{3})"#.parse().unwrap();
    static ref HEADER_LINE_REGEX: Regex = r#"^([^:]+): *([^\r]*)\r\n$"#.parse().unwrap();
}

pub fn parse_status_line(line: &[u8]) -> Option<(Version, StatusCode)> {
    STATUS_LINE_REGEX.captures(line).and_then(|captures| Some((
        match &captures[1] {
            b"HTTP/2" => Version::HTTP_2,
            b"HTTP/1.1" => Version::HTTP_11,
            b"HTTP/1.0" => Version::HTTP_10,
            b"HTTP/0.9" => Version::HTTP_09,
            _ => Version::default(),
        },
        StatusCode::from_bytes(&captures[2]).ok()?,
    )))
}

pub fn parse_header(line: &[u8]) -> Option<(HeaderName, HeaderValue)> {
    HEADER_LINE_REGEX.captures(line).and_then(|captures| Some((
        HeaderName::from_bytes(&captures[1]).ok()?,
        HeaderValue::from_bytes(&captures[2]).ok()?,
    )))
}
