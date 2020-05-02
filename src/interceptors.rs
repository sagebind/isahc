use crate::{Body, Error};
use futures_util::future::BoxFuture;
use http::{Request, Response};
use std::{future::Future, sync::Arc};

type InterceptorResult = Result<Response<Body>, Box<dyn std::error::Error>>;
pub type InterceptorFuture<'a> = BoxFuture<'a, InterceptorResult>;

pub struct Context<'a> {
    pub(crate) invoker: Arc<dyn (Fn(Request<Body>) -> BoxFuture<'a, Result<Response<Body>, Error>>) + Send + Sync + 'a>,
    pub(crate) interceptors: &'a [Box<dyn Interceptor>],
}

impl<'a> Context<'a> {
    /// Send a request.
    pub async fn send(&mut self, request: Request<Body>) -> Result<Response<Body>, Error> {
        if let Some(interceptor) = self.interceptors.first() {
            let inner_context = Self {
                invoker: self.invoker.clone(),
                interceptors: &self.interceptors[1..],
            };
            Ok(interceptor.intercept(request, inner_context).await.unwrap())
        } else {
            (self.invoker)(request).await
        }
    }
}

/// Base trait for middleware.
///
/// Since clients may be used to send requests concurrently, all middleware must
/// be synchronized and must be able to account for multiple requests being made
/// in parallel.
pub trait Interceptor: Send + Sync + 'static {
    /// Intercept a request, returning a response.
    fn intercept<'a>(&self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a>;
}

impl<F> Interceptor for F
where
    F: for<'a> private::AsyncFn2<Request<Body>, Context<'a>, Output = InterceptorResult> + Send + Sync + 'static,
{
    fn intercept<'a>(&self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a> {
        Box::pin(self.call(request, cx))
    }
}

// Workaround for https://github.com/rust-lang/rust/issues/51004
#[allow(unreachable_pub)]
mod private {
    use std::future::Future;

    macro_rules! impl_async_fn {
        ($(($FnOnce:ident, $FnMut:ident, $Fn:ident, ($($arg:ident: $arg_ty:ident,)*)),)*) => {
            $(
                pub trait $FnOnce<$($arg_ty,)*> {
                    type Output;
                    type Future: Future<Output = Self::Output> + Send;
                    fn call_once(self, $($arg: $arg_ty,)*) -> Self::Future;
                }
                pub trait $FnMut<$($arg_ty,)*>: $FnOnce<$($arg_ty,)*> {
                    fn call_mut(&mut self, $($arg: $arg_ty,)*) -> Self::Future;
                }
                pub trait $Fn<$($arg_ty,)*>: $FnMut<$($arg_ty,)*> {
                    fn call(&self, $($arg: $arg_ty,)*) -> Self::Future;
                }
                impl<$($arg_ty,)* F, Fut> $FnOnce<$($arg_ty,)*> for F
                where
                    F: FnOnce($($arg_ty,)*) -> Fut,
                    Fut: Future + Send,
                {
                    type Output = Fut::Output;
                    type Future = Fut;
                    fn call_once(self, $($arg: $arg_ty,)*) -> Self::Future {
                        self($($arg,)*)
                    }
                }
                impl<$($arg_ty,)* F, Fut> $FnMut<$($arg_ty,)*> for F
                where
                    F: FnMut($($arg_ty,)*) -> Fut,
                    Fut: Future + Send,
                {
                    fn call_mut(&mut self, $($arg: $arg_ty,)*) -> Self::Future {
                        self($($arg,)*)
                    }
                }
                impl<$($arg_ty,)* F, Fut> $Fn<$($arg_ty,)*> for F
                where
                    F: Fn($($arg_ty,)*) -> Fut,
                    Fut: Future + Send,
                {
                    fn call(&self, $($arg: $arg_ty,)*) -> Self::Future {
                        self($($arg,)*)
                    }
                }
            )*
        }
    }

    impl_async_fn! {
        (AsyncFnOnce0, AsyncFnMut0, AsyncFn0, ()),
        (AsyncFnOnce1, AsyncFnMut1, AsyncFn1, (a0:A0, )),
        (AsyncFnOnce2, AsyncFnMut2, AsyncFn2, (a0:A0, a1:A1, )),
    }
}
