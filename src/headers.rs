use http::header::HeaderMap;

/// Extension trait for HTTP requests and responses for accessing common headers
/// in a typed way.
///
/// Eventually this trait can be made public once the types are cleaned up a
/// bit.
pub(crate) trait HasHeaders {
    fn headers(&self) -> &HeaderMap;

    fn content_length(&self) -> Option<u64> {
        self.headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
    }

    fn content_type(&self) -> Option<&str> {
        self.headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
    }
}

impl HasHeaders for HeaderMap {
    fn headers(&self) -> &HeaderMap {
        self
    }
}

impl<T> HasHeaders for http::Request<T> {
    fn headers(&self) -> &HeaderMap {
        self.headers()
    }
}

impl<T> HasHeaders for http::Response<T> {
    fn headers(&self) -> &HeaderMap {
        self.headers()
    }
}
