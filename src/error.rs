//! Types for error handling.

use std::{any::TypeId, error::Error as StdError, fmt, io};

/// A list of error types that can occur while sending an HTTP request or
/// receiving an HTTP response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A problem occurred with the local certificate.
    BadClientCertificate,

    /// The server certificate could not be validated.
    BadServerCertificate,

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
    Io(std::io::ErrorKind),

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
    Unknown,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestBodyNotRewindable => {
                f.write_str("request body could not be re-sent because it is not rewindable")
            }
            _ => f.write_str("unknown error"),
        }
    }
}

/// An error encountered while sending an HTTP request or receiving an HTTP
/// response.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

// /// All possible types of errors that can be returned from Isahc.
// #[derive(Debug)]
// #[non_exhaustive]
// pub enum Error {
//     /// The request was aborted before it could be completed.
//     Aborted,
//     /// A problem occurred with the local certificate.
//     BadClientCertificate(Option<String>),
//     /// The server certificate could not be validated.
//     BadServerCertificate(Option<String>),
//     /// Failed to connect to the server.
//     ConnectFailed,
//     /// Couldn't resolve host name.
//     CouldntResolveHost,
//     /// Couldn't resolve proxy host name.
//     CouldntResolveProxy,
//     /// Unrecognized or bad content encoding returned by the server.
//     InvalidContentEncoding(Option<String>),
//     /// Provided credentials were rejected by the server.
//     InvalidCredentials,
//     /// Validation error when constructing the request or parsing the response.
//     InvalidHttpFormat(http::Error),
//     /// Invalid UTF-8 string error.
//     InvalidUtf8,
//     /// An unknown I/O error.
//     Io(io::Error),
//     /// The server did not send a response.
//     NoResponse,
//     /// The server does not support or accept range requests.
//     RangeRequestUnsupported,
//     /// An error occurred while writing the request body.
//     RequestBodyError(Option<String>),
//     /// An error occurred while reading the response body.
//     ResponseBodyError(Option<String>),
//     /// Failed to connect over a secure socket.
//     SSLConnectFailed(Option<String>),
//     /// An error ocurred in the secure socket engine.
//     SSLEngineError(Option<String>),
//     /// An ongoing request took longer than the configured timeout time.
//     Timeout,
//     /// Number of redirects hit the maximum amount.
//     TooManyRedirects,

//     Other(Box<dyn StdError + Send + Sync>),
// }

impl Error {
    pub(crate) fn new(kind: ErrorKind, source: impl StdError + Send + Sync + 'static) -> Self {
        let source: Box<dyn StdError + Send + Sync + 'static> = Box::new(source);

        match source.downcast::<Self>() {
            Ok(e) => *e,
            Err(e) => Self {
                kind,
                source: Some(e),
            },
        }
    }

    pub(crate) fn from_any<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        if TypeId::of::<E>() == TypeId::of::<Self>() {}
        Self::new(ErrorKind::Unknown, error)
    }

    /// Get the kind of error this represents.
    #[inline]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    // pub fn is_tls(&self) -> bool {
    //     self.is_bad_client_certificate() || self.is_bad_server_certificate()
    // }

    // pub fn is_bad_client_certificate(&self) -> bool {
    //     if let Self::Other(e) = self {
    //         if let Some(e) = e.downcast_ref::<curl::Error>() {
    //             return e.is_ssl_certproblem() || e.is_ssl_cacert_badfile();
    //         }
    //     }

    //     false
    // }

    // pub fn is_bad_server_certificate(&self) -> bool {
    //     if let Self::Other(e) = self {
    //         if let Some(e) = e.downcast_ref::<curl::Error>() {
    //             return e.is_peer_failed_verification() || e.is_ssl_cacert();
    //         }
    //     }

    //     false
    // }

    // pub fn is_timeout(&self) -> bool {
    //     match self {
    //         Self::Io(e) => e.kind() == io::ErrorKind::TimedOut,
    //         Self::Other(e) => {
    //             if let Some(e) = e.downcast_ref::<curl::Error>() {
    //                 e.is_operation_timedout()
    //             } else {
    //                 false
    //             }
    //         }
    //         _ => false,
    //     }
    // }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|e| &**e as _)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let extra_description = self
            .source
            .as_ref()
            .and_then(|e| e.downcast_ref::<curl::Error>())
            .and_then(|e| e.extra_description());

        if let Some(s) = extra_description {
            write!(f, "{}: {}", self.kind, s)
        } else {
            write!(f, "{}", self.kind)
        }
        // match self {
        //     Self::Aborted => f.write_str("request aborted unexpectedly"),
        //     Self::BadClientCertificate(Some(ref e)) => f.write_str(e),
        //     Self::BadServerCertificate(Some(ref e)) => f.write_str(e),
        //     Self::ConnectFailed => f.write_str("failed to connect to the server"),
        //     Self::CouldntResolveHost => f.write_str("couldn't resolve host name"),
        //     Self::CouldntResolveProxy => f.write_str("couldn't resolve proxy host name"),
        //     Self::InvalidContentEncoding(Some(ref e)) => write!(f, "invalid content encoding: {}", e),
        //     Self::InvalidCredentials => f.write_str("credentials were rejected by the server"),
        //     Self::InvalidHttpFormat(ref e) => e.fmt(f),
        //     Self::InvalidUtf8 => f.write_str("bytes are not valid UTF-8"),
        //     Self::Io(ref e) => e.fmt(f),
        //     Self::NoResponse => f.write_str("server did not send a response"),
        //     Self::RangeRequestUnsupported => f.write_str("server does not support or accept range requests"),
        //     Self::RequestBodyError(Some(ref e)) => f.write_str(e),
        //     Self::ResponseBodyError(Some(ref e)) => f.write_str(e),
        //     Self::SSLConnectFailed(Some(ref e)) => f.write_str(e),
        //     Self::SSLEngineError(Some(ref e)) => f.write_str(e),
        //     Self::Timeout => f.write_str("request took longer than the configured timeout"),
        //     Self::TooManyRedirects => f.write_str("max redirect limit exceeded"),
        //     Self::Other(ref e) => e.fmt(f),
        //     _ => f.write_str("unknown error"),
        // }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self { kind, source: None }
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
            ErrorKind::Io(std::io::ErrorKind::Other)
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
                // io::ErrorKind::ConnectionRefused => Error::ConnectFailed,
                io::ErrorKind::TimedOut => ErrorKind::Timeout,
                kind => ErrorKind::Io(kind),
            },
            error,
        )
    }
}

impl From<Error> for io::Error {
    fn from(mut error: Error) -> Self {
        // If this error was directly created from an IO error, return it
        // directly.
        if matches!(error.kind, ErrorKind::Io(_)) {
            if let Some(source) = error.source.take() {
                match source.downcast() {
                    Ok(e) => return *e,
                    Err(e) => error.source = Some(e),
                }
            }
        }

        let kind = match error.kind {
            ErrorKind::ConnectionFailed => io::ErrorKind::ConnectionRefused,
            ErrorKind::Timeout => io::ErrorKind::TimedOut,
            _ => io::ErrorKind::Other,
        };

        Self::new(kind, error)
    }
}

#[doc(hidden)]
impl From<curl::MultiError> for Error {
    fn from(error: curl::MultiError) -> Error {
        Self::new(if error.is_bad_socket() {
            ErrorKind::Io(io::ErrorKind::BrokenPipe)
        } else {
            ErrorKind::Unknown
        }, error)
    }
}

#[doc(hidden)]
impl From<http::Error> for Error {
    fn from(error: http::Error) -> Error {
        Self::new(ErrorKind::Protocol, error)
    }
}
