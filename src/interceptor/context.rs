use crate::{Body, Error};
use super::{Interceptor, InterceptorFuture, InterceptorObj};
use http::{Request, Response};
use std::{
    fmt,
    sync::Arc,
};

/// Execution context for an interceptor.
pub struct Context<'a> {
    pub(crate) invoker: Arc<dyn Invoke + Send + Sync + 'a>,
    pub(crate) interceptors: &'a [InterceptorObj],
}

impl<'a> Context<'a> {
    /// Send a request asynchronously, executing the next interceptor in the
    /// chain, if any.
    pub async fn send(&self, request: Request<Body>) -> Result<Response<Body>, Error> {
        if let Some(interceptor) = self.interceptors.first() {
            let inner_context = Self {
                invoker: self.invoker.clone(),
                interceptors: &self.interceptors[1..],
            };

            match interceptor.intercept(request, inner_context).await {
                Ok(response) => Ok(response),

                // If the error is an Isahc error, return it directly.
                Err(e) => match e.downcast::<Error>() {
                    Ok(e) => Err(*e),

                    // TODO: Introduce a new error variant for errors caused by an
                    // interceptor. This is a temporary hack.
                    Err(e) => Err(Error::Curl(e.to_string())),
                },
            }
        } else {
            self.invoker.invoke(request).await
        }
    }
}

impl fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").finish()
    }
}

pub(crate) trait Invoke {
    fn invoke<'a>(&'a self, request: Request<Body>) -> InterceptorFuture<'a, Error>;
}
