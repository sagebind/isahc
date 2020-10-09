use super::CookieJar2;
use http::Response;

pub trait CookieAwareResponse {
    /// Get cookies for this response.
    ///
    /// This returns any cookies that have been recorded for the current HTTP
    /// client session that are applicable to this response, not just cookies
    /// that were explicitly returned by the server in this response.
    fn cookies(&self);

    fn cookie_jar2(&self) -> Option<&dyn CookieJar2>;

    /// Get cookies received directly from the server.
    fn set_cookies(&self);
}

impl<T> CookieAwareResponse for Response<T> {
    fn cookies(&self) {

    }

    fn cookie_jar2(&self) -> Option<&dyn CookieJar2> {
        self.extensions().get::<Box<dyn CookieJar2 + Send + Sync + 'static>>().map(|boxed| &**boxed as &dyn CookieJar2)
    }

    fn set_cookies(&self) {
        if let Some(jar) = self.cookie_jar2() {
            jar.get(self.effective_uri())
        }
    }
}
