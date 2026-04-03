use crate::{
    auth::Authentication,
    body::AsyncBody,
    config::{request::RequestConfig, RedirectPolicy},
    error::{Error, ErrorKind},
    handler::RequestBody,
    interceptor::{Context, Interceptor, InterceptorFuture},
    request::RequestExt,
};
use http::{header::ToStrError, uri::Scheme, HeaderMap, HeaderValue, Request, Response, Uri};
use std::{borrow::Cow, convert::TryFrom, fmt::Write, str};
use url::Url;

/// How many redirects to follow by default if a limit is not specified. We
/// don't actually allow infinite redirects as that could result in a dangerous
/// infinite loop, so by default we actually limit redirects to a large amount.
const DEFAULT_REDIRECT_LIMIT: u32 = 1024;

/// Extension containing the final "effective" URI that was visited, after
/// following any redirects.
#[derive(Clone)]
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
                if let Some(redirect_location) = get_redirect_location(&effective_uri, &response) {
                    // If we've reached the limit, return an error as requested.
                    if redirect_count >= limit {
                        return Err(Error::with_response(ErrorKind::TooManyRedirects, &response));
                    }

                    // Set referer header.
                    if auto_referer {
                        if let Some(referer) = create_referer(&effective_uri, &redirect_location) {
                            if let Some(headers) = request_builder.headers_mut() {
                                headers.insert(http::header::REFERER, referer);
                            }
                        }
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

                    // If we are redirecting to a different authority, scrub
                    // sensitive headers from subsequent requests.
                    if !is_same_authority(&effective_uri, &redirect_location) {
                        if let Some(headers) = request_builder.headers_mut() {
                            scrub_sensitive_headers(headers);
                        }

                        // Remove auth configuration.
                        if let Some(extensions) = request_builder.extensions_mut() {
                            extensions.remove::<Authentication>();
                        }
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
                    effective_uri = redirect_location.clone();
                    request = request_builder
                        .uri(redirect_location)
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
                        write!(&mut s, "%{:02x}", byte).unwrap();
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

/// Create a `Referer` header value to include in a redirected request, if
/// possible and appropriate.
fn create_referer(uri: &Uri, target_uri: &Uri) -> Option<HeaderValue> {
    // Do not set a Referer header if redirecting to an insecure location from a
    // secure one.
    if uri.scheme() == Some(&Scheme::HTTPS) && target_uri.scheme() != Some(&Scheme::HTTPS) {
        return None;
    }

    let mut referer = String::new();

    if let Some(scheme) = uri.scheme() {
        referer.push_str(scheme.as_str());
        referer.push_str("://");
    }

    if let Some(authority) = uri.authority() {
        referer.push_str(authority.host());

        if let Some(port) = authority.port() {
            referer.push(':');
            referer.push_str(port.as_str());
        }
    }

    referer.push_str(uri.path());

    if let Some(query) = uri.query() {
        referer.push('?');
        referer.push_str(query);
    }

    HeaderValue::try_from(referer).ok()
}

fn is_same_authority(a: &Uri, b: &Uri) -> bool {
    a.scheme() == b.scheme() && a.host() == b.host() && a.port() == b.port()
}

fn scrub_sensitive_headers(headers: &mut HeaderMap) {
    headers.remove(http::header::AUTHORIZATION);
    headers.remove(http::header::COOKIE);
    headers.remove("cookie2");
    headers.remove(http::header::PROXY_AUTHORIZATION);
    headers.remove(http::header::WWW_AUTHENTICATE);
}

#[cfg(test)]
mod tests {
    use http::Response;
    use test_case::test_case;

    #[test_case("http://foo.com", "http://foo.com", "http://foo.com/")]
    #[test_case("http://foo.com", "/two", "http://foo.com/two")]
    #[test_case("http://foo.com", "http://foo.com#foo", "http://foo.com/")]
    fn resolve_redirect_location(request_uri: &str, location: &str, resolved: &str) {
        let response = Response::builder()
            .status(301)
            .header("Location", location)
            .body(())
            .unwrap();

        assert_eq!(
            super::get_redirect_location(&request_uri.parse().unwrap(), &response)
                .unwrap()
                .to_string(),
            resolved
        );
    }

    #[test_case(
        "http://example.org/Overview.html",
        "http://example.org/Overview.html",
        Some("http://example.org/Overview.html")
    )]
    #[test_case(
        "http://example.org/#heading",
        "http://example.org/#heading",
        Some("http://example.org/")
    )]
    #[test_case(
        "http://user:pass@example.org",
        "http://user:pass@example.org",
        Some("http://example.org/")
    )]
    #[test_case("https://example.com", "http://example.org", None)]
    fn create_referer_from_uri(uri: &str, target_uri: &str, referer: Option<&str>) {
        assert_eq!(
            super::create_referer(&uri.parse().unwrap(), &target_uri.parse().unwrap())
                .as_ref()
                .and_then(|value| value.to_str().ok()),
            referer
        );
    }

    #[test_case("http://example.com", "http://example.com", true)]
    #[test_case("http://example.com", "http://example.com/foo", true)]
    #[test_case("http://example.com", "http://user:pass@example.com", true)]
    #[test_case("http://example.com", "http://example.com:9000", false)]
    #[test_case("http://example.com:9000", "http://example.com:9000", true)]
    #[test_case("http://example.com", "http://example.org", false)]
    #[test_case("http://example.com", "https://example.com", false)]
    #[test_case("http://example.com", "http://www.example.com", false)]
    fn is_same_authority(a: &str, b: &str, expected: bool) {
        assert_eq!(
            super::is_same_authority(&a.parse().unwrap(), &b.parse().unwrap()),
            expected
        );
    }
}
