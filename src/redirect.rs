use crate::{
    config::RedirectPolicy,
    handler::RequestBody,
    interceptor::{Context, Interceptor, InterceptorFuture},
    request::RequestExt,
    Body,
    Error,
};
use http::{Request, Uri};

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

    fn intercept<'a>(&'a self, request: Request<Body>, ctx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
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

            let auto_referer = request.extensions()
                .get::<crate::config::redirect::AutoReferer>()
                .is_some();

            // Make a copy of the request before sending it.
            let mut request_builder = request.to_builder();

            // Send the request to get the ball rolling.
            let mut response = ctx.send(request).await?;

            let mut redirect_count: u32 = 0;
            let limit = match policy {
                RedirectPolicy::Limit(limit) => limit,
                _ => DEFAULT_REDIRECT_LIMIT,
            };

            // Check for redirects. If a redirect should happen, then curl will
            // return a URI to redirect to, which the request handler will
            // attach to the response as this extension.
            while let Some(RedirectUri(redirect_uri)) = response.extensions_mut().remove::<RedirectUri>() {
                // Sanity check.
                if !response.status().is_redirection() {
                    break;
                }

                // If we've reached the limit, return an error as requested.
                if redirect_count >= limit {
                    return Err(Error::TooManyRedirects);
                }

                // Set referer header.
                if auto_referer {
                    let referer = request_builder.uri_ref().unwrap().to_string();
                    request_builder = request_builder.header(http::header::REFERER, referer);
                }

                // Check if we should change the request method into a GET. HTTP
                // specs don't really say one way or another when this should
                // happen for most status codes, so we just mimic curl's
                // behavior here since it is so common.
                if response.status() == 301 || response.status() == 302 || response.status() == 303 {
                    request_builder = request_builder.method(http::Method::GET);
                }

                // Grab the request body back from the internal handler, as we
                // might need to send it again (if possible...)
                let mut request_body = response.extensions_mut()
                    .remove::<RequestBody>()
                    .map(|v| v.0)
                    .unwrap_or_default();

                // Redirect handling is tricky when we are uploading something.
                // If we can, reset the body stream to the beginning. This might
                // work if the body to upload is an in-memory byte buffer, but
                // for arbitrary streams we can't do this.
                //
                // There's not really a good way of handling this gracefully, so
                // we just return an error so that the user knows about it.
                if !request_body.reset() {
                    return Err(Error::RequestBodyError(Some(String::from("could not follow redirect because request body is not rewindable"))));
                }

                let request = request_builder.uri(redirect_uri)
                    .body(request_body)?;

                // Keep another clone of the request around again, in case we
                // need to follow yet another redirect.
                request_builder = request.to_builder();

                // Send the redirected request.
                response = ctx.send(request).await?;

                redirect_count += 1;
            }

            Ok(response)
        })
    }
}
