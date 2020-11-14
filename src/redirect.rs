use crate::{
    config::RedirectPolicy,
    handler::RequestBody,
    interceptor::{Context, Interceptor, InterceptorFuture},
    request::RequestExt,
    Body, Error,
};
use http::{Request, Response, Uri};
use std::convert::TryFrom;
use url::Url;

/// How many redirects to follow by default if a limit is not specified. We
/// don't actually allow infinite redirects as that could result in a dangerous
/// infinite loop, so by default we actually limit redirects to a large amount.
const DEFAULT_REDIRECT_LIMIT: u32 = 1024;

/// Extension containing the final "effective" URI that was visited, after
/// following any redirects.
pub(crate) struct EffectiveUri(pub(crate) Uri);

/// Interceptor that implements automatic following of HTTP redirects.
pub(crate) struct RedirectInterceptor;

impl Interceptor for RedirectInterceptor {
    type Err = Error;

    fn intercept<'a>(
        &'a self,
        mut request: Request<Body>,
        ctx: Context<'a>,
    ) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(async move {
            // Store the effective URI to include in the response.
            let mut effective_uri = request.uri().clone();

            // Get the redirect policy for this request.
            let policy = request
                .extensions()
                .get::<RedirectPolicy>()
                .cloned()
                .unwrap_or_default();

            // No redirect handling, just proceed normally.
            if policy == RedirectPolicy::None {
                let mut response = ctx.send(request).await?;
                response
                    .extensions_mut()
                    .insert(EffectiveUri(effective_uri));

                return Ok(response);
            }

            let auto_referer = request
                .extensions()
                .get::<crate::config::redirect::AutoReferer>()
                .is_some();

            let limit = match policy {
                RedirectPolicy::Limit(limit) => limit,
                _ => DEFAULT_REDIRECT_LIMIT,
            };

            // Keep track of how many redirects we've done.
            let mut redirect_count: u32 = 0;

            loop {
                // Preserve a clone of the request before sending it.
                let mut request_builder = request.to_builder();

                // Send the request to get the ball rolling.
                let mut response = ctx.send(request).await?;

                // Check for a redirect.
                if let Some(location) = get_redirect_location(&effective_uri, &response) {
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
                    if response.status() == 301 || response.status() == 302 || response.status() == 303
                    {
                        request_builder = request_builder.method(http::Method::GET);
                    }

                    // Grab the request body back from the internal handler, as we
                    // might need to send it again (if possible...)
                    let mut request_body = response
                        .extensions_mut()
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
                        return Err(Error::RequestBodyError(Some(String::from(
                            "could not follow redirect because request body is not rewindable",
                        ))));
                    }

                    // Update the request to point to the new URI.
                    effective_uri = location.clone();
                    request = request_builder.uri(location).body(request_body)?;
                    redirect_count += 1;
                }

                // No more redirects; set the effective URI we finally settled on and return.
                else {
                    response
                        .extensions_mut()
                        .insert(EffectiveUri(effective_uri));

                    return Ok(response);
                }
            }
        })
    }
}

fn get_redirect_location<T>(request_uri: &Uri, response: &Response<T>) -> Option<Uri> {
    if response.status().is_redirection() {
        let location = response.headers().get(http::header::LOCATION)?;

        match location.to_str() {
            Ok(location) => {
                match resolve(request_uri, location) {
                    Ok(uri) => return Some(uri),
                    Err(e) => {
                        tracing::debug!("bad redirect location: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("bad redirect location: {}", e);
            }
        }
    }

    None
}

/// Resolve one URI in terms of another.
fn resolve(base: &Uri, target: &str) -> Result<Uri, Box<dyn std::error::Error>> {
    // Optimistically check if this is an absolute URI.
    match Url::parse(target) {
        Ok(url) => Ok(Uri::try_from(url.as_str())?),

        // Relative URI, resolve against the base.
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let base = Url::parse(base.to_string().as_str())?;

            Ok(Uri::try_from(base.join(target)?.as_str())?)
        }

        Err(e) => Err(Box::new(e)),
    }
}
