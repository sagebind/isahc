//! HTTP client interceptor API.
//!
//! This module provides the core types and functions for defining and working
//! with interceptors. Interceptors are handlers that augment HTTP client
//! functionality by decorating HTTP calls with custom logic.

use crate::{Body, Error};
use futures_util::future::BoxFuture;
use http::{Request, Response};
use std::{
    error::Error as StdError,
    fmt,
    sync::Arc,
};

/// Defines an inline interceptor using a closure-like syntax.
///
/// Closures are not supported due to a limitation in Rust's type inference.
#[cfg(feature = "unstable-interceptors")]
#[macro_export]
macro_rules! interceptor {
    ($request:ident, $cx:ident, $body:expr) => {{
        async fn interceptor(
            mut $request: $crate::http::Request<$crate::Body>,
            $cx: $crate::interceptors::Context<'_>,
        ) -> Result<$crate::http::Response<isahc::Body>, Box<dyn std::error::Error>> {
            (move || async move {
                $body
            })().await.map_err(Into::into)
        }

        interceptor
    }};
}

/// Base trait for interceptors.
///
/// Since clients may be used to send requests concurrently, all interceptors
/// must be synchronized and must be able to account for multiple requests being
/// made in parallel.
pub trait Interceptor: Send + Sync {
    /// The type of error returned by this interceptor.
    type Err: Into<Box<dyn StdError>>;

    /// Intercept a request, returning a response.
    ///
    /// The returned future is allowed to borrow the interceptor for the
    /// duration of its execution.
    fn intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Self::Err>;
}

/// The type of future returned by an interceptor.
pub type InterceptorFuture<'a, E> = BoxFuture<'a, Result<Response<Body>, E>>;

/// Type-erased interceptor object.
pub(crate) struct InterceptorObj<'a>(Box<dyn DynInterceptor + 'a>);

impl<'a> InterceptorObj<'a> {
    pub(crate) fn new(interceptor: impl Interceptor + 'a) -> Self {
        Self(Box::new(interceptor))
    }
}

impl Interceptor for InterceptorObj<'_> {
    type Err = Box<dyn StdError>;

    fn intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
        self.0.dyn_intercept(request, cx)
    }
}

/// Object-safe version of the interceptor used for type erasure.
trait DynInterceptor: Send + Sync {
    fn dyn_intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Box<dyn StdError>>;
}

impl<I> DynInterceptor for I
where
    I: Interceptor
{
    fn dyn_intercept<'a>(&'a self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Box<dyn StdError>> {
        Box::pin(async move {
            self.intercept(request, cx).await.map_err(Into::into)
        })
    }
}

impl<E, F> Interceptor for F
where
    E: Into<Box<dyn StdError>>,
    F: for<'a> private::AsyncFn2<Request<Body>, Context<'a>, Output = Result<Response<Body>, E>> + Send + Sync + 'static,
{
    type Err = E;

    fn intercept<'a>(&self, request: Request<Body>, cx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(self.call(request, cx))
    }
}

/// Execution context for an interceptor.
pub struct Context<'a> {
    pub(crate) invoker: Arc<dyn (Fn(Request<Body>) -> InterceptorFuture<'a, Error>) + Send + Sync + 'a>,
    pub(crate) interceptors: &'a [InterceptorObj<'static>],
}

impl Context<'_> {
    /// Send a request.
    pub async fn send(&self, request: Request<Body>) -> Result<Response<Body>, Error> {
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

impl fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").finish()
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
