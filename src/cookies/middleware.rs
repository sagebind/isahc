//! Middleware layer that provides automatic cookie session management for any
//! request with an attached cookie jar.

use super::{Cookie, CookieJar};
use crate::{middleware::Middleware, response::ResponseExt, Body};
use http::{Request, Response};

#[derive(Debug, Default)]
pub(crate) struct CookieMiddleware;

impl Middleware for CookieMiddleware {
    fn filter_request(&self, mut request: Request<Body>) -> Request<Body> {
        if let Some(jar) = request.extensions().get::<CookieJar>() {
            if let Some(header) = jar.get_cookies(request.uri()) {
                request
                .headers_mut()
                .insert(http::header::COOKIE, header.parse().unwrap());
            }
        }

        request
    }

    /// Extracts cookies set via the Set-Cookie header.
    fn filter_response(&self, response: Response<Body>) -> Response<Body> {
        // TODO: Clone cookie jar from request into response.
        if let Some(jar) = response.extensions().get::<CookieJar>() {
            if response.headers().contains_key(http::header::SET_COOKIE) {
                let cookies = response
                    .headers()
                    .get_all(http::header::SET_COOKIE)
                    .into_iter()
                    .filter_map(|header| {
                        header.to_str().ok().or_else(|| {
                            tracing::warn!("invalid encoding in Set-Cookie header");
                            None
                        })
                    })
                    .filter_map(|header| {
                        response
                            .effective_uri()
                            .and_then(|uri| Cookie::parse(header, uri))
                            .or_else(|| {
                                tracing::warn!("could not parse Set-Cookie header");
                                None
                            })
                    });

                jar.add(cookies);
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Uri;

    #[test]
    fn cookie_lifecycle() {
        let uri: Uri = "https://example.com/foo".parse().unwrap();
        let jar = CookieJar::default();
        let middleware = CookieMiddleware::default();

        middleware.filter_response(
            http::Response::builder()
                .header(http::header::SET_COOKIE, "foo=bar")
                .header(http::header::SET_COOKIE, "baz=123")
                .extension(crate::response::EffectiveUri(uri.clone()))
                .extension(jar.clone())
                .body(crate::Body::default())
                .unwrap(),
        );

        let request = middleware.filter_request(
            http::Request::builder()
                .uri(uri)
                .extension(jar.clone())
                .body(crate::Body::default())
                .unwrap(),
        );

        assert_eq!(request.headers()[http::header::COOKIE], "baz=123; foo=bar");
    }
}
