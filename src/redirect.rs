use crate::{
    body::AsyncBody,
    config::{request::RequestConfig, RedirectPolicy},
    error::{Error, ErrorKind},
    handler::RequestBody,
    interceptor::{Context, Interceptor, InterceptorFuture},
    request::RequestExt,
};
use http::{header::ToStrError, HeaderValue, Request, Response, Uri};
use std::{borrow::Cow, convert::TryFrom, str};
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
        mut request: Request<AsyncBody>,
        ctx: Context<'a>,
    ) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(async move {
            // Store the effective URI to include in the response.
            let mut effective_uri = request.uri().clone();

            // Get the redirect policy for this request.
            let policy = request
                .extensions()
                .get::<RequestConfig>()
                .and_then(|config| config.redirect_policy.as_ref())
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
                .get::<RequestConfig>()
                .and_then(|config| config.auto_referer)
                .unwrap_or(false);

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
                        return Err(Error::with_response(ErrorKind::TooManyRedirects, &response));
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
                    if response.status() == 301
                        || response.status() == 302
                        || response.status() == 303
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
                        return Err(Error::with_response(
                            ErrorKind::RequestBodyNotRewindable,
                            &response,
                        ));
                    }

                    // Update the request to point to the new URI.
                    effective_uri = location.clone();
                    request = request_builder
                        .uri(location)
                        .body(request_body)
                        .map_err(|e| Error::new(ErrorKind::InvalidRequest, e))?;
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

        match parse_location(location) {
            Ok(location) => match resolve(request_uri, location.as_ref()) {
                Ok(uri) => return Some(uri),
                Err(e) => {
                    tracing::debug!("invalid redirect location: {}", e);
                }
            },
            Err(e) => {
                tracing::debug!("invalid redirect location: {}", e);
            }
        }
    }

    None
}

/// Parse the given `Location` header value into a string.
fn parse_location(location: &HeaderValue) -> Result<Cow<'_, str>, ToStrError> {
    match location.to_str() {
        // This will return a `str` if the header value contained only legal
        // US-ASCII characters.
        Ok(s) => Ok(Cow::Borrowed(s)),

        // Try to parse the location as UTF-8 bytes instead of US-ASCII. This is
        // technically against the URI spec which requires all literal
        // characters in the URI to be US-ASCII (see [RFC 3986, Section
        // 4.1](https://tools.ietf.org/html/rfc3986#section-4.1)).
        //
        // This is also more or less against the HTTP spec, which historically
        // allowed for ISO-8859-1 text in header values but since was restricted
        // to US-ASCII plus opaque bytes. Never has UTF-8 been encouraged or
        // allowed as-such. See [RFC 7230, Section
        // 3.2.4](https://tools.ietf.org/html/rfc7230#section-3.2.4) for more
        // info.
        //
        // However, some bad or misconfigured web servers will do this anyway,
        // and most web browsers recover from this by allowing and interpreting
        // UTF-8 characters as themselves even though they _should_ have been
        // percent-encoded. The third-party URI parsers that we use have no such
        // leniency, so we percent-encode such bytes (if legal UTF-8) ahead of
        // time before handing them off to the URI parser.
        Err(e) => {
            if str::from_utf8(location.as_bytes()).is_ok() {
                let mut s = String::with_capacity(location.len());

                for &byte in location.as_bytes() {
                    if byte.is_ascii() {
                        s.push(byte as char);
                    } else {
                        s.push_str(&format!("%{:02x}", byte));
                    }
                }

                Ok(Cow::Owned(s))
            } else {
                // Header value isn't legal UTF-8 either, not much we can
                // reasonably do.
                Err(e)
            }
        }
    }
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
