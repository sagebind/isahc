use super::{Interceptor, InterceptorFuture, InterceptorObj};
use crate::{body::AsyncBody, error::Error};
use http::{Request, Response};
use std::{fmt, sync::Arc};

/// Execution context for an interceptor.
pub struct Context<'a> {
    pub(crate) invoker: Arc<dyn Invoke + Send + Sync + 'a>,
    pub(crate) interceptors: &'a [InterceptorObj],
}

impl<'a> Context<'a> {
    /// Send a request asynchronously, executing the next interceptor in the
    /// chain, if any.
    pub async fn send(&self, request: Request<AsyncBody>) -> Result<Response<AsyncBody>, Error> {
        if let Some(interceptor) = self.interceptors.first() {
            let inner_context = Self {
                invoker: self.invoker.clone(),
                interceptors: &self.interceptors[1..],
            };

            interceptor.intercept(request, inner_context).await
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
    fn invoke(&self, request: Request<AsyncBody>) -> InterceptorFuture<'_, Error>;
}
