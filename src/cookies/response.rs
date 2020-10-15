use super::CookieJar;
use http::Response;

pub trait CookieAwareResponse {
    /// Get cookies for this response.
    ///
    /// This returns any cookies that have been recorded for the current HTTP
    /// client session that are applicable to this response, not just cookies
    /// that were explicitly returned by the server in this response.
    fn cookies(&self);

    /// Get the value of a cookie by name.
    fn cookie(&self, name: &str) -> Option<String>;

    /// Get the cookie jar associated with this response, if any.
    ///
    /// The cookie jar is set either by the request that produced this response
    /// or the [`HttpClient`](crate::HttpClient) used to make the request.
    fn cookie_jar(&self) -> Option<&CookieJar>;

    /// Get cookies received directly from the server.
    fn set_cookies(&self);
}

impl<T> CookieAwareResponse for Response<T> {
    fn cookies(&self) {

    }

    fn cookie(&self, name: &str) -> Option<String> {
        None
    }

    fn cookie_jar(&self) -> Option<&CookieJar> {
        self.extensions().get()
    }

    fn set_cookies(&self) {
        if let Some(jar) = self.cookie_jar() {
            // jar.get(self.effective_uri())
        }
    }
}
