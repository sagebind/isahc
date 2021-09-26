//! Types for error handling.

use std::{error::Error as StdError, fmt, io, net::SocketAddr, sync::Arc};

use http::Response;
use once_cell::sync::OnceCell;

use crate::ResponseExt;

/// A non-exhaustive list of error types that can occur while sending an HTTP
/// request or receiving an HTTP response.
///
/// These are meant to be treated as general error codes that allow you to
/// handle different sorts of errors in different ways, but are not always
/// specific. The list is also non-exhaustive, and more variants may be added in
/// the future.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A problem occurred with the local certificate.
    BadClientCertificate,

    /// The server certificate could not be validated.
    BadServerCertificate,

    /// The HTTP client failed to initialize.
    ///
    /// This error can occur when trying to create a client with invalid
    /// configuration, if there were insufficient resources to create the
    /// client, or if a system error occurred when trying to initialize an I/O
    /// driver.
    ClientInitialization,

    /// Failed to connect to the server. This can occur if the server rejects
    /// the request on the specified port.
    ConnectionFailed,

    /// The server either returned a response using an unknown or unsupported
    /// encoding format, or the response encoding was malformed.
    InvalidContentEncoding,

    /// Provided authentication credentials were rejected by the server.
    ///
    /// This error is only returned when using Isahc's built-in authentication
    /// methods. If using authentication headers manually, the server's response
    /// will be returned as a success unaltered.
    InvalidCredentials,

    /// The request to be sent was invalid and could not be sent.
    ///
    /// Note that this is only returned for requests that the client deemed
    /// invalid. If the request appears to be valid but is rejected by the
    /// server, then the server's response will likely indicate as such.
    InvalidRequest,

    /// An I/O error either sending the request or reading the response. This
    /// could be caused by a problem on the client machine, a problem on the
    /// server machine, or a problem with the network between the two.
    ///
    /// You can get more details about the underlying I/O error with
    /// [`Error::source`][std::error::Error::source].
    Io,

    /// Failed to resolve a host name.
    ///
    /// This could be caused by any number of problems, including failure to
    /// reach a DNS server, misconfigured resolver configuration, or the
    /// hostname simply does not exist.
    NameResolution,

    /// The server made an unrecoverable HTTP protocol violation. This indicates
    /// a bug in the server. Retrying a request that returns this error is
    /// likely to produce the same error.
    ProtocolViolation,

    /// Request processing could not continue because the client needed to
    /// re-send the request body, but was unable to rewind the body stream to
    /// the beginning in order to do so.
    ///
    /// If you need Isahc to be able to re-send the request body during a retry
    /// or redirect then you must load the body into a contiguous memory buffer
    /// first. Then you can create a rewindable body using
    /// [`Body::from_bytes_static`][crate::Body::from_bytes_static] or
    /// [`AsyncBody::from_bytes_static`][crate::AsyncBody::from_bytes_static].
    RequestBodyNotRewindable,

    /// A request or operation took longer than the configured timeout time.
    Timeout,

    /// An error ocurred in the secure socket engine.
    TlsEngine,

    /// Number of redirects hit the maximum configured amount.
    TooManyRedirects,

    /// An unknown error occurred. This likely indicates a problem in the HTTP
    /// client or in a dependency, but the client was able to recover instead of
    /// panicking. Subsequent requests will likely succeed.
    ///
    /// Only used internally.
    #[doc(hidden)]
    Unknown,
}

impl ErrorKind {
    #[inline]
    fn description(&self) -> Option<&str> {
        match self {
            Self::BadClientCertificate => Some("a problem occurred with the local certificate"),
            Self::BadServerCertificate => Some("the server certificate could not be validated"),
            Self::ClientInitialization => Some("failed to initialize client"),
            Self::ConnectionFailed => Some("failed to connect to the server"),
            Self::InvalidContentEncoding => Some(
                "the server either returned a response using an unknown or unsupported encoding format, or the response encoding was malformed",
            ),
            Self::InvalidCredentials => {
                Some("provided authentication credentials were rejected by the server")
            }
            Self::InvalidRequest => Some("invalid HTTP request"),
            Self::NameResolution => Some("failed to resolve host name"),
            Self::ProtocolViolation => {
                Some("the server made an unrecoverable HTTP protocol violation")
            }
            Self::RequestBodyNotRewindable => {
                Some("request body could not be re-sent because it is not rewindable")
            }
            Self::Timeout => {
                Some("request or operation took longer than the configured timeout time")
            }
            Self::TlsEngine => Some("error ocurred in the secure socket engine"),
            Self::TooManyRedirects => Some("number of redirects hit the maximum amount"),
            _ => None,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description().unwrap_or("unknown error"))
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
///
/// Note that errors are typically caused by failed I/O or protocol errors. 4xx
/// or 5xx responses successfully received from the server are generally _not_
/// considered an error case.
///
/// This type is intentionally opaque, as sending an HTTP request involves many
/// different moving parts, some of which can be platform or device-dependent.
/// It is recommended that you use the [`kind`][Error::kind] method to get a
/// more generalized classification of error types that this error could be if
/// you need to handle different sorts of errors in different ways.
///
/// If you need to get more specific details about the reason for the error, you
/// can use the [`source`][std::error::Error::source] method. We do not provide
/// any stability guarantees about what error sources are returned.
#[derive(Clone)]
pub struct Error(Arc<Inner>);

struct Inner {
    kind: ErrorKind,
    context: Option<String>,
    source: Option<Box<dyn SourceError>>,
    local_addr: OnceCell<SocketAddr>,
    remote_addr: OnceCell<SocketAddr>,
}

impl Error {
    /// Create a new error from a given error kind and source error.
    pub(crate) fn new<E>(kind: ErrorKind, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::with_context(kind, None, source)
    }

    /// Create a new error from a given error kind, source error, and context
    /// string.
    pub(crate) fn with_context<E>(kind: ErrorKind, context: Option<String>, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self(Arc::new(Inner {
            kind,
            context,
            source: Some(Box::new(source)),
            local_addr: OnceCell::new(),
            remote_addr: OnceCell::new(),
        }))
    }

    /// Create a new error from a given error kind and response.
    pub(crate) fn with_response<B>(kind: ErrorKind, response: &Response<B>) -> Self {
        let error = Self::from(kind);

        if let Some(addr) = response.local_addr() {
            let _ = error.0.local_addr.set(addr);
        }

        if let Some(addr) = response.remote_addr() {
            let _ = error.0.remote_addr.set(addr);
        }

        error
    }

    /// Statically cast a given error into an Isahc error, converting if
    /// necessary.
    ///
    /// This is useful for converting or creating errors from external types
    /// without publicly implementing `From` over them and leaking them into our
    /// API.
    pub(crate) fn from_any<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        castaway::match_type!(error, {
            Error as error => error,
            std::io::Error as error => error.into(),
            curl::Error as error => {
                Self::with_context(
                    if error.is_ssl_certproblem() || error.is_ssl_cacert_badfile() {
                        ErrorKind::BadClientCertificate
                    } else if error.is_peer_failed_verification()
                        || error.is_ssl_cacert()
                        || error.is_ssl_cipher()
                        || error.is_ssl_issuer_error()
                    {
                        ErrorKind::BadServerCertificate
                    } else if error.is_interface_failed() {
                        ErrorKind::ClientInitialization
                    } else if error.is_couldnt_connect() || error.is_ssl_connect_error() {
                        ErrorKind::ConnectionFailed
                    } else if error.is_bad_content_encoding() || error.is_conv_failed() {
                        ErrorKind::InvalidContentEncoding
                    } else if error.is_login_denied() {
                        ErrorKind::InvalidCredentials
                    } else if error.is_url_malformed() {
                        ErrorKind::InvalidRequest
                    } else if error.is_couldnt_resolve_host() || error.is_couldnt_resolve_proxy() {
                        ErrorKind::NameResolution
                    } else if error.is_got_nothing()
                        || error.is_http2_error()
                        || error.is_http2_stream_error()
                        || error.is_unsupported_protocol()
                        || error.code() == curl_sys::CURLE_FTP_WEIRD_SERVER_REPLY
                    {
                        ErrorKind::ProtocolViolation
                    } else if error.is_send_error()
                        || error.is_recv_error()
                        || error.is_read_error()
                        || error.is_write_error()
                        || error.is_upload_failed()
                        || error.is_send_fail_rewind()
                        || error.is_aborted_by_callback()
                        || error.is_partial_file()
                    {
                        ErrorKind::Io
                    } else if error.is_ssl_engine_initfailed()
                        || error.is_ssl_engine_notfound()
                        || error.is_ssl_engine_setfailed()
                    {
                        ErrorKind::TlsEngine
                    } else if error.is_operation_timedout() {
                        ErrorKind::Timeout
                    } else if error.is_too_many_redirects() {
                        ErrorKind::TooManyRedirects
                    } else {
                        ErrorKind::Unknown
                    },
                    error.extra_description().map(String::from),
                    error,
                )
            },
            curl::MultiError as error => {
                Self::new(
                    if error.is_bad_socket() {
                        ErrorKind::Io
                    } else {
                        ErrorKind::Unknown
                    },
                    error,
                )
            },
            error => Error::new(ErrorKind::Unknown, error),
        })
    }

    /// Get the kind of error this represents.
    ///
    /// The kind returned may not be matchable against any documented variants
    /// if the reason for the error is unknown. Unknown errors may be an
    /// indication of a bug, or an error condition that we do not recognize
    /// appropriately. Either way, please report such occurrences to us!
    #[inline]
    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }

    /// Returns true if this error was likely caused by the client.
    ///
    /// Usually indicates that the client was misconfigured or used to send
    /// invalid data to the server. Requests that return these sorts of errors
    /// probably should not be retried without first fixing the request
    /// parameters.
    pub fn is_client(&self) -> bool {
        match self.kind() {
            ErrorKind::BadClientCertificate
            | ErrorKind::ClientInitialization
            | ErrorKind::InvalidCredentials
            | ErrorKind::InvalidRequest
            | ErrorKind::RequestBodyNotRewindable
            | ErrorKind::TlsEngine => true,
            _ => false,
        }
    }

    /// Returns true if this is an error likely related to network failures.
    ///
    /// Network operations are inherently unreliable. Sometimes retrying the
    /// request once or twice is enough to resolve the error.
    pub fn is_network(&self) -> bool {
        match self.kind() {
            ErrorKind::ConnectionFailed | ErrorKind::Io | ErrorKind::NameResolution => true,
            _ => false,
        }
    }

    /// Returns true if this error was likely the fault of the server.
    pub fn is_server(&self) -> bool {
        match self.kind() {
            ErrorKind::BadServerCertificate
            | ErrorKind::ProtocolViolation
            | ErrorKind::TooManyRedirects => true,
            _ => false,
        }
    }

    /// Returns true if this error is caused from exceeding a configured
    /// timeout.
    ///
    /// A request could time out for any number of reasons, for example:
    ///
    /// - Slow or broken network preventing the server from receiving the
    ///   request or replying in a timely manner.
    /// - The server received the request but is taking a long time to fulfill
    ///   the request.
    ///
    /// Sometimes retrying the request once or twice is enough to resolve the
    /// error.
    pub fn is_timeout(&self) -> bool {
        self.kind() == ErrorKind::Timeout
    }

    /// Returns true if this error is related to SSL/TLS.
    pub fn is_tls(&self) -> bool {
        match self.kind() {
            ErrorKind::BadClientCertificate
            | ErrorKind::BadServerCertificate
            | ErrorKind::TlsEngine => true,
            _ => false,
        }
    }

    /// Get the local socket address of the last-used connection involved in
    /// this error, if known.
    ///
    /// If the request that caused this error failed to create a local socket
    /// for connecting then this will return `None`.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.0.local_addr.get().cloned()
    }

    /// Get the remote socket address of the last-used connection involved in
    /// this error, if known.
    ///
    /// If the request that caused this error failed to connect to any server,
    /// then this will return `None`.
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.0.remote_addr.get().cloned()
    }

    pub(crate) fn with_local_addr(self, addr: SocketAddr) -> Self {
        let _ = self.0.local_addr.set(addr);
        self
    }

    pub(crate) fn with_remote_addr(self, addr: SocketAddr) -> Self {
        let _ = self.0.remote_addr.set(addr);
        self
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source.as_ref().map(|source| source.as_dyn_error())
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
            .field("context", &self.0.context)
            .field("source", &self.source())
            .field(
                "source_type",
                &self.0.source.as_ref().map(|e| e.type_name()),
            )
            .field("local_addr", &self.0.local_addr.get())
            .field("remote_addr", &self.0.remote_addr.get())
            .finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = self.0.context.as_ref() {
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
            context: None,
            source: None,
            local_addr: OnceCell::new(),
            remote_addr: OnceCell::new(),
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

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Error {
        Self::new(
            if error.is::<http::header::InvalidHeaderName>()
                || error.is::<http::header::InvalidHeaderValue>()
                || error.is::<http::method::InvalidMethod>()
                || error.is::<http::uri::InvalidUri>()
                || error.is::<http::uri::InvalidUriParts>()
            {
                ErrorKind::InvalidRequest
            } else {
                ErrorKind::Unknown
            },
            error,
        )
    }
}

/// Internal trait object for source errors. This is used to capture additional
/// methods about the source error value in the vtable.
trait SourceError: StdError + Send + Sync + 'static {
    /// Get the type name of the concrete error type when the parent error was
    /// created. Used for enriching the debug formatting.
    fn type_name(&self) -> &'static str;

    /// Cast this error as a stdlib error trait object.
    fn as_dyn_error(&self) -> &(dyn StdError + 'static);
}

impl<T: StdError + Send + Sync + 'static> SourceError for T {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn as_dyn_error(&self) -> &(dyn StdError + 'static) {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(Error: Send, Sync);
}
