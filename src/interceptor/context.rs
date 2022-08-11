use super::{Interceptor, InterceptorFuture};
use crate::{body::AsyncBody, error::Error, HttpClient};
use http::{Request, Response};
use std::fmt;

/// Execution context for an interceptor.
pub struct Context {
    pub(crate) client: HttpClient,
    pub(crate) interceptor_offset: usize,
}

impl Context {
    /// Send a request asynchronously, executing the next interceptor in the
    /// chain, if any.
    pub async fn send(&self, request: Request<AsyncBody>) -> Result<Response<AsyncBody>, Error> {
        if let Some(interceptor) = self.client.interceptors().get(self.interceptor_offset) {
            let inner_context = Self {
                client: self.client.clone(),
                interceptor_offset: self.interceptor_offset + 1,
            };

            interceptor.intercept(request, inner_context).await
        } else {
            self.client.invoke(request).await
        }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").finish()
    }
}

pub(crate) trait Invoke {
    fn invoke(&self, request: Request<AsyncBody>) -> InterceptorFuture<'_, Error>;
}
