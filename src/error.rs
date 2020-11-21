//! Types for error handling.

use std::{error::Error as StdError, fmt, io, sync::Arc};

/// An error fault, describing whether an error was caused by the HTTP client or
/// by the HTTP server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Fault {
    /// The client was misconfigured or used to send invalid data to the server.
    ///
    /// Requests that return these sorts of errors probably should not be
    /// retried.
    Client,

    /// The server behaved incorrectly.
    Server,
}

/// A non-exhaustive list of error types that can occur while sending an HTTP
/// request or receiving an HTTP response.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A problem occurred with the local certificate.
    BadClientCertificate,

    /// The server certificate could not be validated.
    BadServerCertificate,

    /// The HTTP client failed to initialize.
    ClientInitialization,

    /// Failed to connect to the server.
    ConnectionFailed,

    /// Couldn't resolve host name.
    CouldntResolveHost,

    /// Couldn't resolve proxy host name.
    CouldntResolveProxy,

    /// The server either returned a response using an unknown or unsupported
    /// encoding format, or the response encoding was malformed.
    InvalidContentEncoding,

    /// Provided authentication credentials were rejected by the server.
    InvalidCredentials,

    /// An I/O error either sending the request or reading the response. This
    /// could be caused by a problem on the client machine, a problem on the
    /// server machine, or a problem with the network between the two.
    Io,

    /// The server made an unrecoverable HTTP protocol violation. This indicates
    /// a bug in the server. Retrying a request that returns this error is
    /// likely to produce the same error.
    Protocol,

    /// Request processing could not continue because the client needed to
    /// re-send the request body, but was unable to rewind the body stream to
    /// the beginning in order to do so.
    RequestBodyNotRewindable,

    /// A request or operation took longer than the configured timeout time.
    Timeout,

    /// An error ocurred in the secure socket engine.
    TlsEngineError,

    /// Number of redirects hit the maximum amount.
    TooManyRedirects,

    /// An unknown error occurred. This likely indicates a problem in the HTTP
    /// client or in a dependency, but the client was able to recover instead of
    /// panicking. Subsequent requests will likely succeed.
    #[doc(hidden)]
    Unknown,
}

impl ErrorKind {
    /// Returns true if this is an error related to the network.
    pub fn is_network(&self) -> bool {
        match self {
            Self::ConnectionFailed | Self::CouldntResolveHost | Self::Timeout => true,
            _ => false,
        }
    }

    /// Get the fault for this error kind. Returns `None` if the fault is
    /// unclear.
    pub fn fault(&self) -> Option<Fault> {
        match self {
            Self::BadClientCertificate | Self::InvalidCredentials => Some(Fault::Client),
            Self::BadServerCertificate | Self::Protocol => Some(Fault::Server),
            _ => None,
        }
    }

    /// Returns true if this error is related to SSL/TLS.
    pub fn is_tls(&self) -> bool {
        match self {
            Self::BadClientCertificate | Self::BadServerCertificate | Self::TlsEngineError => true,
            _ => false,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadClientCertificate => f.write_str("a problem occurred with the local certificate"),
            Self::BadServerCertificate => f.write_str("the server certificate could not be validated"),
            Self::ClientInitialization => f.write_str("failed to initialize client"),
            Self::ConnectionFailed => f.write_str("failed to connect to the server"),
            Self::CouldntResolveHost => f.write_str("couldn't resolve host name"),
            Self::CouldntResolveProxy => f.write_str("couldn't resolve proxy host name"),
            Self::InvalidContentEncoding => f.write_str("the server either returned a response using an unknown or unsupported encoding format, or the response encoding was malformed"),
            Self::InvalidCredentials => f.write_str("provided authentication credentials were rejected by the server"),
            Self::Protocol => f.write_str("the server made an unrecoverable HTTP protocol violation"),
            Self::RequestBodyNotRewindable => f.write_str("request body could not be re-sent because it is not rewindable"),
            Self::Timeout => f.write_str("request or operation took longer than the configured timeout time"),
            Self::TlsEngineError => f.write_str("error ocurred in the secure socket engine"),
            Self::TooManyRedirects => f.write_str("number of redirects hit the maximum amount"),
            _ => f.write_str("unknown error"),
        }
    }
}

// Improve equality ergonomics for references.
impl PartialEq<ErrorKind> for &'_ ErrorKind {
    fn eq(&self, other: &ErrorKind) -> bool {
        *self == other
    }
}

/// An error encountered while sending an HTTP request or receiving an HTTP
/// response.
#[derive(Clone)]
pub struct Error(Arc<Inner>);

struct Inner {
    kind: ErrorKind,
    extra: Option<String>,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl Error {
    /// Create a new error from a given error kind and source error.
    pub(crate) fn new<E>(kind: ErrorKind, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self(Arc::new(Inner {
            kind,
            extra: None,
            source: Some(Box::new(source)),
        }))
    }

    /// Statically cast a given error into an Isahc error, converting if
    /// necessary.
    pub(crate) fn from_any<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        match_type! {
            <error as Error> => error,
            <error as std::io::Error> => error.into(),
            error => Error::new(ErrorKind::Unknown, error),
        }
    }

    /// Get the kind of error this represents.
    #[inline]
    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source.as_ref().map(|source| &**source as _)
    }
}

impl PartialEq<ErrorKind> for Error {
    fn eq(&self, kind: &ErrorKind) -> bool {
        self.kind().eq(kind)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind())
            .field("source", &self.source())
            .finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let extra_description = self
            .source()
            .and_then(|e| e.downcast_ref::<curl::Error>())
            .and_then(|e| e.extra_description())
            .or_else(|| self.0.extra.as_deref());

        if let Some(s) = extra_description {
            write!(f, "{}: {}", self.kind(), s)
        } else {
            write!(f, "{}", self.kind())
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self(Arc::new(Inner {
            kind,
            extra: None,
            source: None,
        }))
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        // If this I/O error is just a wrapped Isahc error, then unwrap it.
        if let Some(inner) = error.get_ref() {
            if inner.is::<Self>() {
                return *error.into_inner().unwrap().downcast().unwrap();
            }
        }

        Self::new(
            match error.kind() {
                io::ErrorKind::ConnectionRefused => ErrorKind::ConnectionFailed,
                io::ErrorKind::TimedOut => ErrorKind::Timeout,
                _ => ErrorKind::Io,
            },
            error,
        )
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> Self {
        let kind = match error.kind() {
            ErrorKind::ConnectionFailed => io::ErrorKind::ConnectionRefused,
            ErrorKind::Timeout => io::ErrorKind::TimedOut,
            _ => io::ErrorKind::Other,
        };

        Self::new(kind, error)
    }
}

#[doc(hidden)]
impl From<curl::Error> for Error {
    fn from(error: curl::Error) -> Error {
        let kind = if error.is_ssl_certproblem() || error.is_ssl_cacert_badfile() {
            ErrorKind::BadClientCertificate
        } else if error.is_peer_failed_verification() || error.is_ssl_cacert() {
            ErrorKind::BadServerCertificate
        } else if error.is_couldnt_connect() || error.is_ssl_connect_error() {
            ErrorKind::ConnectionFailed
        } else if error.is_couldnt_resolve_host() {
            ErrorKind::CouldntResolveHost
        } else if error.is_couldnt_resolve_proxy() {
            ErrorKind::CouldntResolveProxy
        } else if error.is_bad_content_encoding() || error.is_conv_failed() {
            ErrorKind::InvalidContentEncoding
        } else if error.is_login_denied() {
            ErrorKind::InvalidCredentials
        } else if error.is_got_nothing() {
            ErrorKind::Protocol
        } else if error.is_read_error()
            || error.is_write_error()
            || error.is_aborted_by_callback()
            || error.is_partial_file()
            || error.is_interface_failed()
        {
            ErrorKind::Io
        } else if error.is_ssl_engine_initfailed()
            || error.is_ssl_engine_notfound()
            || error.is_ssl_engine_setfailed()
        {
            ErrorKind::TlsEngineError
        } else if error.is_operation_timedout() {
            ErrorKind::Timeout
        } else if error.is_too_many_redirects() {
            ErrorKind::TooManyRedirects
        } else {
            ErrorKind::Unknown
        };

        Self::new(kind, error)
    }
}

#[doc(hidden)]
impl From<curl::MultiError> for Error {
    fn from(error: curl::MultiError) -> Error {
        Self::new(
            if error.is_bad_socket() {
                ErrorKind::Io
            } else {
                ErrorKind::Unknown
            },
            error,
        )
    }
}

#[doc(hidden)]
impl From<http::Error> for Error {
    fn from(error: http::Error) -> Error {
        Self::new(ErrorKind::Unknown, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(Error: Send, Sync);
}
