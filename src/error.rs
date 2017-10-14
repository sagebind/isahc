use curl;
use std::error::Error as StdError;
use std::fmt;


#[derive(Debug)]
pub enum Error {
    TransportBusy,
    CurlError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Error::description(self).fmt(f)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        "unknown error"
    }
}

impl From<curl::Error> for Error {
    fn from(error: curl::Error) -> Error {
        Error::CurlError(error.description().to_owned())
    }
}

impl From<curl::MultiError> for Error {
    fn from(error: curl::MultiError) -> Error {
        Error::CurlError(error.description().to_owned())
    }
}

