use curl;
use http;
use std::error::Error as StdError;
use std::fmt;
use std::io;


#[derive(Debug)]
pub enum Error {
    Curl(String),
    InvalidHttpFormat(http::Error),
    InvalidJson,
    InvalidUtf8,
    Io(io::Error),
    TransportBusy,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Error::description(self).fmt(f)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Curl(ref e) => e,
            &Error::InvalidHttpFormat(ref e) => e.description(),
            &Error::InvalidJson => "body is not valid JSON",
            &Error::InvalidUtf8 => "bytes are not valid UTF-8",
            &Error::Io(ref e) => e.description(),
            &Error::TransportBusy => "transport is already in use",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            &Error::InvalidHttpFormat(ref e) => Some(e),
            &Error::Io(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<curl::Error> for Error {
    fn from(error: curl::Error) -> Error {
        Error::Curl(error.description().to_owned())
    }
}

impl From<curl::MultiError> for Error {
    fn from(error: curl::MultiError) -> Error {
        Error::Curl(error.description().to_owned())
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Error {
        Error::InvalidHttpFormat(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

impl From<::std::string::FromUtf8Error> for Error {
    fn from(_: ::std::string::FromUtf8Error) -> Error {
        Error::InvalidUtf8
    }
}

#[cfg(feature = "json")]
impl From<::json::Error> for Error {
    fn from(error: ::json::Error) -> Error {
        match error {
            ::json::Error::FailedUtf8Parsing => Error::InvalidUtf8,
            _ => Error::InvalidJson,
        }
    }
}
