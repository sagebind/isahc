//! Interceptor that provides automatic cookie session management for any
//! request with an attached cookie jar.

use super::{Cookie, CookieJar};
use crate::{
    interceptor::{Context, Interceptor, InterceptorFuture},
    response::ResponseExt,
    Body,
    Error,
};
use http::Request;

#[derive(Debug)]
pub(crate) struct CookieInterceptor {
    /// Default cookie jar to use for all requests.
    cookie_jar: CookieJar,
}

impl CookieInterceptor {
    pub(crate) fn new(cookie_jar: CookieJar) -> Self {
        Self {
            cookie_jar,
        }
    }
}

impl Interceptor for CookieInterceptor {
    type Err = Error;

    fn intercept<'a>(&'a self, mut request: Request<Body>, ctx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(async move {
            // Determine the cookie jar to use for this request. If one is
            // attached to this specific request, use it, otherwise use the
            // default one.
            let jar = request.extensions().get::<CookieJar>()
                .cloned()
                .unwrap_or_else(|| self.cookie_jar.clone());

            // Set the outgoing cookie header.
            if let Some(header) = jar.get_cookies(request.uri()) {
                // TODO: Don't clobber any manually-set cookies already present.
                request
                    .headers_mut()
                    .insert(http::header::COOKIE, header.parse().unwrap());
            }

            let request_uri = request.uri().clone();
            let mut response = ctx.send(request).await?;

            // Persist cookies returned from the server, if any.
            if response.headers().contains_key(http::header::SET_COOKIE) {
                let request_uri = response.effective_uri()
                    .unwrap_or(&request_uri);

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
                    .filter_map(|header| header.parse::<Cookie>().ok().or_else(|| {
                        tracing::warn!("could not parse Set-Cookie header");
                        None
                    }));

                for cookie in cookies {
                    jar.set(cookie, request_uri);
                }
            }

            // Attach cookie jar to response for user inspection.
            response.extensions_mut().insert(jar);

            Ok(response)
        })
    }
}
