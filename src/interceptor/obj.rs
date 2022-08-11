use super::{Context, Interceptor, InterceptorFuture};
use crate::{body::AsyncBody, error::Error};
use http::Request;

/// Type-erased interceptor object.
pub(crate) struct InterceptorObj(Box<dyn DynInterceptor>);

impl<I> From<I> for InterceptorObj
where
    I: Interceptor + 'static,
{
    fn from(interceptor: I) -> Self {
        Self(Box::new(interceptor))
    }
}

impl Interceptor for &InterceptorObj {
    type Err = Error;

    fn intercept(
        &self,
        request: Request<AsyncBody>,
        cx: Context,
    ) -> InterceptorFuture<'_, Self::Err> {
        self.0.dyn_intercept(request, cx)
    }
}

/// Object-safe version of the interceptor used for type erasure. Implementation
/// detail of [`InterceptorObj`].
trait DynInterceptor: Send + Sync {
    fn dyn_intercept(
        &self,
        request: Request<AsyncBody>,
        cx: Context,
    ) -> InterceptorFuture<'_, Error>;
}

impl<I: Interceptor> DynInterceptor for I {
    fn dyn_intercept(
        &self,
        request: Request<AsyncBody>,
        cx: Context,
    ) -> InterceptorFuture<'_, Error> {
        Box::pin(async move { self.intercept(request, cx).await.map_err(Error::from_any) })
    }
}
