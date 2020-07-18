//! Configuration for customizing how connections are established and sockets
//! are opened.

use super::SetOpt;
use curl::easy::Easy2;
use http::Uri;
use std::{
    convert::TryFrom,
    fmt,
    path::Path,
    str::FromStr,
};

#[cfg(feature = "unstable-dial-ip")]
use std::net::SocketAddr;

/// An error which can be returned when parsing a dial address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialParseError(());

impl fmt::Display for DialParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("invalid dial address syntax")
    }
}

impl std::error::Error for DialParseError {}

/// A custom address or dialer for connecting to a host.
///
/// # Examples
///
/// Connect to a UNIX socket:
///
/// ```
/// use isahc::config::Dial;
///
/// let unix_socket = "unix://path/to/my.sock".parse::<Dial>()?;
/// # Ok::<(), isahc::config::DialParseError>(())
/// ```
#[derive(Debug)]
pub struct Dial(Inner);

#[derive(Debug, Eq, PartialEq)]
enum Inner {
    Default,
    #[cfg(feature = "unstable-dial-ip")]
    IpSocket(String),
    UnixSocket(String),
}

impl Dial {
    /// Connect to a UNIX socket described by a file.
    pub fn unix_socket(path: impl AsRef<Path>) -> Result<Self, DialParseError> {
        if let Some(s) = path.as_ref().to_str() {
            Ok(Self(Inner::UnixSocket(s.to_owned())))
        } else {
            Err(DialParseError(()))
        }
    }

    /// Connect to the given IP socket.
    #[cfg(feature = "unstable-dial-ip")]
    pub fn addr(socket_addr: impl Into<SocketAddr>) -> Self {
        // Create a string in the format CURLOPT_CONNECT_TO expects.
        Self(Inner::IpSocket(format!("::{}", socket_addr.into())))
    }
}

impl Default for Dial {
    fn default() -> Self {
        Self(Inner::Default)
    }
}

#[cfg(feature = "unstable-dial-ip")]
impl From<SocketAddr> for Dial {
    fn from(socket_addr: SocketAddr) -> Self {
        Self::addr(socket_addr)
    }
}

impl FromStr for Dial {
    type Err = DialParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[cfg(feature = "unstable-dial-ip")]
        if s.starts_with("tcp:") {
            let addr_str = s[4..].trim_start_matches("/");

            return addr_str
                .parse::<SocketAddr>()
                .map(Self::addr)
                .map_err(|_| DialParseError(()));
        }

        if s.starts_with("unix:") {
            return Self::unix_socket(&s[5..]);
        }

        Err(DialParseError(()))
    }
}

impl TryFrom<&'_ str> for Dial {
    type Error = DialParseError;

    fn try_from(str: &str) -> Result<Self, Self::Error> {
        str.parse()
    }
}

impl TryFrom<String> for Dial {
    type Error = DialParseError;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        string.parse()
    }
}

impl TryFrom<Uri> for Dial {
    type Error = DialParseError;

    fn try_from(uri: Uri) -> Result<Self, Self::Error> {
        // Not the most efficient implementation, but straightforward.
        uri.to_string().parse()
    }
}

impl SetOpt for Dial {
    #[allow(unsafe_code)]
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match &self.0 {
            // Do nothing, as promised.
            Inner::Default => Ok(()),

            // TODO: Make safe interface upstream.
            #[cfg(feature = "unstable-dial-ip")]
            Inner::IpSocket(addr) => unsafe {
                let data = std::ffi::CString::new(addr.as_str())?;

                // TODO: This leaks.
                let slist = curl_sys::curl_slist_append(std::ptr::null_mut(), data.as_ptr());

                match curl_sys::curl_easy_setopt(easy.raw(), 243, slist) {
                    curl_sys::CURLE_OK => Ok(()),
                    code => Err(curl::Error::new(code)),
                }
            }

            // Set the UNIX socket path.
            Inner::UnixSocket(path) => easy.unix_socket(path.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_unix_socket_uri() {
        let dial = "unix://path/to/my.sock".parse::<Dial>().unwrap();

        assert_eq!(dial.0, Inner::UnixSocket("//path/to/my.sock".to_owned()));
    }
}
