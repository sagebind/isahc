use http::{
    header::{HeaderName, HeaderValue},
    StatusCode,
    Version,
};

pub(crate) fn parse_status_line(line: &[u8]) -> Option<(Version, StatusCode)> {
    let mut parts = line.split(u8::is_ascii_whitespace);

    let version = match parts.next()? {
        b"HTTP/3" => Version::HTTP_3,
        b"HTTP/2" => Version::HTTP_2,
        b"HTTP/1.1" => Version::HTTP_11,
        b"HTTP/1.0" => Version::HTTP_10,
        b"HTTP/0.9" => Version::HTTP_09,
        _ => return None,
    };

    let status_code = parts
        .find(|s| !s.is_empty())
        .map(StatusCode::from_bytes)?
        .ok()?;

    Some((version, status_code))
}

pub(crate) fn parse_header(line: &[u8]) -> Option<(HeaderName, HeaderValue)> {
    let split_index = line.iter().position(|&f| f == b':')?;

    let name = HeaderName::from_bytes(&line[..split_index]).ok()?;
    let mut value = &line[split_index + 1..];

    // Trim whitespace
    while let Some((byte, right)) = value.split_first() {
        if byte.is_ascii_whitespace() {
            value = right;
        } else {
            break;
        }
    }

    while let Some((byte, left)) = value.split_last() {
        if byte.is_ascii_whitespace() {
            value = left;
        } else {
            break;
        }
    }

    let value = HeaderValue::from_bytes(value).ok()?;

    Some((name, value))
}

pub(crate) fn header_to_curl_string(
    name: &HeaderName,
    value: &HeaderValue,
    title_case: bool,
) -> String {
    let header_value = value
        .to_str()
        .expect("request header value is not valid UTF-8!");

    let mut string = String::new();

    if title_case {
        let name_bytes: &[u8] = name.as_ref();
        let mut at_start_of_word = true;

        for &byte in name_bytes {
            if at_start_of_word {
                string.push(byte.to_ascii_uppercase().into());
            } else {
                string.push(byte.into());
            }

            at_start_of_word = !byte.is_ascii_alphanumeric();
        }
    } else {
        string.push_str(name.as_str());
    }

    // libcurl requires a special syntax to set a header with an explicit empty
    // value. See https://curl.haxx.se/libcurl/c/CURLOPT_HTTPHEADER.html.
    if header_value.trim().is_empty() {
        string.push(';');
    } else {
        string.push_str(": ");
        string.push_str(header_value);
    }

    string
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_status_line() {
        assert_eq!(
            parse_status_line(b"HTTP/0.9  200  \r\n"),
            Some((Version::HTTP_09, StatusCode::OK,))
        );
        assert_eq!(
            parse_status_line(b"HTTP/1.0 500 Internal Server Error\r\n"),
            Some((Version::HTTP_10, StatusCode::INTERNAL_SERVER_ERROR,))
        );
        assert_eq!(
            parse_status_line(b"HTTP/1.1 404 not found \r\n"),
            Some((Version::HTTP_11, StatusCode::NOT_FOUND,))
        );
        assert_eq!(
            parse_status_line(b"HTTP/2 200\r\n"),
            Some((Version::HTTP_2, StatusCode::OK,))
        );
        assert_eq!(
            parse_status_line(b"HTTP/3 200\r\n"),
            Some((Version::HTTP_3, StatusCode::OK,))
        );
    }

    #[test]
    fn parse_invalid_status_line() {
        assert_eq!(parse_status_line(b""), None);
        assert_eq!(parse_status_line(b" \r\n"), None);
        assert_eq!(parse_status_line(b"HTP/foo bar baz\r\n"), None);
        assert_eq!(parse_status_line(b"a-header: bar\r\n"), None);
        assert_eq!(
            parse_status_line(b" HTTP/1.1 500 Internal Server Error\r\n"),
            None
        );
        assert_eq!(parse_status_line(b"HTTP/4 200\r\n"), None);
    }

    #[test]
    fn parse_valid_headers() {
        assert_eq!(
            parse_header(b"Empty:"),
            Some(("empty".parse().unwrap(), "".parse().unwrap(),))
        );
        assert_eq!(
            parse_header(b"CONTENT-LENGTH:20\r\n"),
            Some(("content-length".parse().unwrap(), "20".parse().unwrap(),))
        );
        assert_eq!(
            parse_header(b"x-Server:     Rust \r"),
            Some(("x-server".parse().unwrap(), "Rust".parse().unwrap(),))
        );
        assert_eq!(
            parse_header(b"X-val: Hello World\r"),
            Some(("x-val".parse().unwrap(), "Hello World".parse().unwrap(),))
        );

        assert_eq!(
            parse_header(b"Location: https://example.com/\r"),
            Some((
                "location".parse().unwrap(),
                "https://example.com/".parse().unwrap(),
            ))
        );
    }

    #[test]
    fn parse_invalid_headers() {
        assert_eq!(parse_header(b""), None);
        assert_eq!(parse_header(b":"), None);
        assert_eq!(parse_header(b": bar"), None);
        assert_eq!(parse_header(b"a\nheader: bar"), None);
        assert_eq!(parse_header(b"foo : bar\r"), None);
    }

    #[test]
    fn normal_header_to_curl_string() {
        let name = "User-Agent".parse().unwrap();
        let value = "foo".parse().unwrap();

        assert_eq!(
            header_to_curl_string(&name, &value, false),
            "user-agent: foo"
        );
    }

    #[test]
    fn blank_header_to_curl_string() {
        let name = "User-Agent".parse().unwrap();
        let value = "".parse().unwrap();

        assert_eq!(header_to_curl_string(&name, &value, false), "user-agent;");
    }

    #[test]
    fn normal_header_to_curl_string_title_case() {
        let name = "User-Agent".parse().unwrap();
        let value = "foo".parse().unwrap();

        assert_eq!(
            header_to_curl_string(&name, &value, true),
            "User-Agent: foo"
        );
    }
}
