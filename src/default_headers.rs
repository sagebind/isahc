use crate::{
    body::AsyncBody,
    error::Error,
    interceptor::{Context, Interceptor, InterceptorFuture},
};
use http::{HeaderMap, HeaderValue, Request};

/// Interceptor that adds a set of default headers on every outgoing request, if
/// not explicitly set on the request.
pub(crate) struct DefaultHeadersInterceptor {
    headers: HeaderMap<HeaderValue>,
}

impl From<HeaderMap<HeaderValue>> for DefaultHeadersInterceptor {
    fn from(headers: HeaderMap<HeaderValue>) -> Self {
        Self {
            headers,
        }
    }
}

impl Interceptor for DefaultHeadersInterceptor {
    type Err = Error;

    fn intercept<'a>(
        &'a self,
        mut request: Request<AsyncBody>,
        ctx: Context<'a>,
    ) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(async move {
            // We are checking here if header already contains the key, simply
            // ignore it. In case the key wasn't present in parts.headers ensure
            // that we have all the headers from default headers.
            for name in self.headers.keys() {
                if !request.headers().contains_key(name) {
                    for v in self.headers.get_all(name).iter() {
                        request.headers_mut().append(name, v.clone());
                    }
                }
            }

            ctx.send(request).await
        })
    }
}
