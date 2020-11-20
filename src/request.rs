use crate::{
    client::ResponseFuture,
    config::{internal::ConfigurableBase, Configurable},
    {Body, Error},
};
use http::{Request, Response};

/// Extension methods on an HTTP request.
pub trait RequestExt<T> {
    /// Create a new request builder with the method, URI, and headers cloned
    /// from this request.
    ///
    /// Note that third-party extensions are not cloned.
    fn to_builder(&self) -> http::request::Builder;

    /// Send the HTTP request synchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send`](crate::send).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let response = Request::post("https://httpbin.org/post")
    ///     .header("Content-Type", "application/json")
    ///     .body(r#"{
    ///         "speed": "fast",
    ///         "cool_name": true
    ///     }"#)?
    ///     .send()?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>;

    /// Sends the HTTP request asynchronously using the default client.
    ///
    /// This is a convenience method that is equivalent to
    /// [`send_async`](crate::send_async).
    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<Body>;
}

impl<T> RequestExt<T> for Request<T> {
    fn to_builder(&self) -> http::request::Builder {
        let mut builder = Request::builder()
            .method(self.method().clone())
            .uri(self.uri().clone())
            .version(self.version());

        *builder.headers_mut().unwrap() = self.headers().clone();

        // Clone known extensions.
        macro_rules! try_clone_extension {
            ($extensions:expr, $builder:expr, [$($ty:ty,)*]) => {{
                let extensions = $extensions;
                $(
                    if let Some(extension) = extensions.get::<$ty>() {
                        $builder = $builder.extension(extension.clone());
                    }
                )*
            }}
        }

        try_clone_extension!(
            self.extensions(),
            builder,
            [
                crate::config::Timeout,
                crate::config::ConnectTimeout,
                crate::config::TcpKeepAlive,
                crate::config::TcpNoDelay,
                crate::config::NetworkInterface,
                crate::config::Dialer,
                crate::config::RedirectPolicy,
                crate::config::redirect::AutoReferer,
                crate::config::AutomaticDecompression,
                crate::auth::Authentication,
                crate::auth::Credentials,
                crate::config::MaxAgeConn,
                crate::config::MaxUploadSpeed,
                crate::config::MaxDownloadSpeed,
                crate::config::VersionNegotiation,
                crate::config::proxy::Proxy<Option<http::Uri>>,
                crate::config::proxy::Blacklist,
                crate::config::proxy::Proxy<crate::auth::Authentication>,
                crate::config::proxy::Proxy<crate::auth::Credentials>,
                crate::config::DnsCache,
                crate::config::dns::ResolveMap,
                crate::config::dns::Servers,
                crate::config::ssl::Ciphers,
                crate::config::ClientCertificate,
                crate::config::CaCertificate,
                crate::config::SslOption,
                crate::config::CloseConnection,
                crate::config::EnableMetrics,
                crate::config::IpVersion,
            ]
        );

        builder
    }

    fn send(self) -> Result<Response<Body>, Error>
    where
        T: Into<Body>,
    {
        crate::send(self)
    }

    fn send_async(self) -> ResponseFuture<'static>
    where
        T: Into<Body>,
    {
        crate::send_async(self)
    }
}

impl Configurable for http::request::Builder {}

impl ConfigurableBase for http::request::Builder {
    fn configure(self, option: impl Send + Sync + 'static) -> Self {
        self.extension(option)
    }
}
