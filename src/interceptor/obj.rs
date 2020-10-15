use crate::Body;
use super::{Context, Interceptor, InterceptorFuture};
use http::Request;
use std::error::Error;

/// Type-erased interceptor object.
pub(crate) struct InterceptorObj(Box<dyn DynInterceptor>);

impl InterceptorObj {
    pub(crate) fn new(interceptor: impl Interceptor + 'static) -> Self {
        Self(Box::new(interceptor))
    }
}

impl Interceptor for InterceptorObj {
    type Err = Box<dyn Error>;

    fn intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
        self.0.dyn_intercept(request, cx)
    }
}

/// Object-safe version of the interceptor used for type erasure. Implementation
/// detail of [`InterceptorObj`].
trait DynInterceptor: Send + Sync {
    fn dyn_intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Box<dyn Error>>;
}

impl<I: Interceptor> DynInterceptor for I {
    fn dyn_intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Box<dyn Error>> {
        Box::pin(async move {
            self.intercept(request, cx).await.map_err(Into::into)
        })
    }
}
