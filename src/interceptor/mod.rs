//! HTTP client interceptor API.
//!
//! This module provides the core types and functions for defining and working
//! with interceptors. Interceptors are handlers that augment HTTP client
//! functionality by decorating HTTP calls with custom logic.
//!
//! Known issues:
//!
//! - [`from_fn`] doesn't work as desired. The trait bounds are too ambiguous
//!   for the compiler to infer for closures, and since the return type is
//!   generic over a lifetime, there's no way to give the return type the
//!   correct name using current Rust syntax.
//! - [`InterceptorObj`] wraps the returned future in an extra box.
//! - If an interceptor returns a custom error, it is stringified and wrapped in
//!   `Error::Curl`. We should introduce a new error variant that boxes the
//!   error and also records the type of the interceptor that created the error
//!   for visibility. But we can't add a new variant right now without a BC
//!   break. See [#182](https://github.com/sagebind/isahc/issues/182).
//! - Automatic redirect following currently bypasses interceptors for
//!   subsequent requests. This will be fixed when redirect handling is
//!   rewritten as an interceptor itself. See
//!   [#232](https://github.com/sagebind/isahc/issues/232).
///
/// # Availability
///
/// This module is only available when the
/// [`unstable-interceptors`](../index.html#unstable-interceptors) feature is
/// enabled.

use crate::Body;
use http::{Request, Response};
use std::{
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
};

mod context;
mod obj;

pub use self::context::Context;
pub(crate) use self::{
    context::Invoke,
    obj::InterceptorObj,
};

/// Defines an inline interceptor using a closure-like syntax.
///
/// Closures are not supported due to a limitation in Rust's type inference.
#[cfg(feature = "unstable-interceptors")]
#[macro_export]
macro_rules! interceptor {
    ($request:ident, $ctx:ident, $body:expr) => {{
        async fn interceptor(
            mut $request: $crate::http::Request<$crate::Body>,
            $ctx: $crate::interceptor::Context<'_>,
        ) -> Result<$crate::http::Response<isahc::Body>, Box<dyn ::std::error::Error>> {
            (move || async move {
                $body
            })().await.map_err(Into::into)
        }

        $crate::interceptor::from_fn(interceptor)
    }};
}

/// Base trait for interceptors.
///
/// Since clients may be used to send requests concurrently, all interceptors
/// must be synchronized and must be able to account for multiple requests being
/// made in parallel.
pub trait Interceptor: Send + Sync {
    /// The type of error returned by this interceptor.
    type Err: Into<Box<dyn Error>>;

    /// Intercept a request, returning a response.
    ///
    /// The returned future is allowed to borrow the interceptor for the
    /// duration of its execution.
    fn intercept<'a>(&'a self, request: Request<Body>, ctx: Context<'a>) -> InterceptorFuture<'a, Self::Err>;
}

/// The type of future returned by an interceptor.
pub type InterceptorFuture<'a, E> = Pin<Box<dyn Future<Output = Result<Response<Body>, E>> + Send + 'a>>;

/// Creates an interceptor from an arbitrary closure or function.
pub fn from_fn<F, E>(f: F) -> InterceptorFn<F>
where
    F: for<'a> private::AsyncFn2<Request<Body>, Context<'a>, Output = Result<Response<Body>, E>> + Send + Sync + 'static,
    E: Into<Box<dyn Error>>,
{
    InterceptorFn(f)
}

/// An interceptor created from an arbitrary closure or function. See
/// [`from_fn`] for details.
pub struct InterceptorFn<F>(F);

impl<E, F> Interceptor for InterceptorFn<F>
where
    E: Into<Box<dyn Error>>,
    F: for<'a> private::AsyncFn2<Request<Body>, Context<'a>, Output = Result<Response<Body>, E>> + Send + Sync + 'static,
{
    type Err = E;

    fn intercept<'a>(&self, request: Request<Body>, ctx: Context<'a>) -> InterceptorFuture<'a, Self::Err> {
        Box::pin(self.0.call(request, ctx))
    }
}

impl<F: fmt::Debug> fmt::Debug for InterceptorFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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
