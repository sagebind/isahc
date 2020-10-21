use crate::{
    config::RedirectPolicy,
    interceptor::{Context, Interceptor, InterceptorFuture},
    response::ResponseExt,
    Body,
    Error,
};
use http::{Request, Uri};
use std::convert::TryInto;

/// How many redirects to follow by default if a limit is not specified. We
/// don't actually allow infinite redirects as that could result in a dangerous
/// infinite loop, so by default we actually limit redirects to a large amount.
const DEFAULT_REDIRECT_LIMIT: u32 = 1024;

/// Extension containing the redirect target on the response determined by curl,
/// if any.
pub(crate) struct RedirectUri(pub(crate) Uri);

/// Interceptor that implements automatic following of HTTP redirects.
pub(crate) struct RedirectInterceptor;

impl Interceptor for RedirectInterceptor {
    type Err = Error;

    fn intercept<'a>(&'a self, request: &'a mut Request<Body>, ctx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(async move {
            // Get the redirect policy for this request.
            let policy = request.extensions()
                .get::<RedirectPolicy>()
                .cloned()
                .unwrap_or_default();

            // No redirect handling, just proceed normally.
            if policy == RedirectPolicy::None {
                return ctx.send(request).await;
            }

            // Send the request to get the ball rolling.
            let mut response = ctx.send(request).await?;

            let mut redirect_count: u32 = 0;
            let limit = match policy {
                RedirectPolicy::Limit(limit) => limit,
                _ => DEFAULT_REDIRECT_LIMIT,
            };

            // Check for redirects.
            while let Some(RedirectUri(redirect_uri)) = response.extensions_mut().remove::<RedirectUri>() {
                if !response.status().is_redirection() {
                    break;
                }

                if redirect_count >= limit {
                    return Err(Error::TooManyRedirects);
                }

                // Set referer header.
                if request.extensions().get::<crate::config::redirect::AutoReferer>().is_some() {
                    if let Ok(header_value) = request.uri().to_string().try_into() {
                        request.headers_mut().insert(http::header::REFERER, header_value);
                    }
                }

                if response.status() == 303 {
                    *request.method_mut() = http::Method::GET;
                }

                // TODO: Filter out certain headers.
                *request.uri_mut() = redirect_uri;
                response = ctx.send(request).await?;

                redirect_count += 1;
            }

            Ok(response)
        })
    }
}
