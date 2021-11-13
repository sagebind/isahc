//! Interceptor that provides automatic cookie session management for any
//! request with an attached cookie jar.

use super::{Cookie, CookieJar};
use crate::{
    body::AsyncBody,
    error::Error,
    interceptor::{Context, Interceptor, InterceptorFuture},
    response::ResponseExt,
};
use http::Request;
use std::convert::TryInto;

#[derive(Debug)]
pub(crate) struct CookieInterceptor {
    /// Default cookie jar to use for all requests, if any.
    cookie_jar: Option<CookieJar>,
}

impl CookieInterceptor {
    pub(crate) fn new(cookie_jar: Option<CookieJar>) -> Self {
        Self { cookie_jar }
    }
}

impl Interceptor for CookieInterceptor {
    type Err = Error;

    fn intercept<'a>(
        &'a self,
        mut request: Request<AsyncBody>,
        ctx: Context<'a>,
    ) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(async move {
            // Determine the cookie jar to use for this request. If one is
            // attached to this specific request, use it, otherwise use the
            // default one.
            let jar = request
                .extensions()
                .get::<CookieJar>()
                .cloned()
                .or_else(|| self.cookie_jar.clone());

            if let Some(jar) = jar.as_ref() {
                // Get the outgoing cookie header.
                let mut cookie_string = request
                    .headers_mut()
                    .remove(http::header::COOKIE)
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();

                // Append cookies in the jar to the cookie header value.
                for cookie in jar.get_for_uri(request.uri()) {
                    if !cookie_string.is_empty() {
                        cookie_string.extend_from_slice(b"; ");
                    }

                    cookie_string.extend_from_slice(cookie.name().as_bytes());
                    cookie_string.push(b'=');
                    cookie_string.extend_from_slice(cookie.value().as_bytes());
                }

                if !cookie_string.is_empty() {
                    if let Ok(header_value) = cookie_string.try_into() {
                        request
                            .headers_mut()
                            .insert(http::header::COOKIE, header_value);
                    }
                }
            }

            let request_uri = request.uri().clone();
            let mut response = ctx.send(request).await?;

            if let Some(jar) = jar {
                // Persist cookies returned from the server, if any.
                if response.headers().contains_key(http::header::SET_COOKIE) {
                    let request_uri = response.effective_uri().unwrap_or(&request_uri);

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
                            Cookie::parse(header).ok().or_else(|| {
                                tracing::warn!("could not parse Set-Cookie header");
                                None
                            })
                        });

                    for cookie in cookies {
                        let _ = jar.set(cookie, request_uri);
                    }
                }

                // Attach cookie jar to response for user inspection.
                response.extensions_mut().insert(jar);
            }

            Ok(response)
        })
    }
}
