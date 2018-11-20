//! cook

use chashmap::CHashMap;
use http::Uri;
use middleware::Middleware;
use std::sync::Mutex;
use super::{Request, Response};

/// A storage mechanism for persisting cookies between HTTP requests.
pub trait CookieJar {
    fn get(&self, uri: &Uri);

    fn set(&mut self, uri: &Uri);

    fn clear(&mut self);
}

/// An in-memory cookie jar implementation.
#[derive(Default)]
pub struct SessionCookieJar {
    /// A map of cookies indexed by a string.
    cookies: CHashMap<Uri, Vec<()>>,
}

impl CookieJar for SessionCookieJar {
    fn get(&self, uri: &Uri) {
        self.cookies.get(uri);
    }

    fn set(&mut self, uri: &Uri) {
        self.cookies.alter(uri.clone(), |cookies| {
            let mut cookies = cookies.unwrap_or_default();
            cookies.push(());
            Some(cookies)
        });
    }

    fn clear(&mut self) {
        self.cookies.clear();
    }
}

/// Provides automatic cookie session management.
#[derive(Default)]
pub struct CookieMiddleware<J> {
    jar: Mutex<J>,
}

impl<J> From<J> for CookieMiddleware<J> where J: CookieJar {
    fn from(jar: J) -> Self {
        CookieMiddleware {
            jar: Mutex::new(jar),
        }
    }
}

impl<J> Middleware for CookieMiddleware<J> where J: CookieJar + 'static {
    fn before(&self, request: Request) -> Request {
        self.jar.lock().unwrap().clear();

        request
    }

    fn after(&self, response: Response) -> Response {
        response
    }
}
