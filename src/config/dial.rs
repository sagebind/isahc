//! Configuration for customizing how connections are established and sockets
//! are opened.

use super::SetOpt;
use curl::easy::{Easy2, List};
use http::Uri;
use std::{
    convert::TryFrom,
    fmt,
    net::SocketAddr,
    str::FromStr,
};

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
/// A dialer can be created from a URI-like string using [`FromStr`]. The
/// following URI schemes are supported:
///
/// - `tcp`: Connect to a TCP address and port pair, like `tcp:127.0.0.1:8080`.
/// - `unix`: Connect to a Unix socket located on the file system, like
///   `unix:/path/to/my.sock`. This is only supported on Unix.
///
/// # Examples
///
/// Connect to a Unix socket URI:
///
/// ```
/// # #![cfg(unix)]
/// use isahc::config::Dial;
///
/// let unix_socket = "unix:/path/to/my.sock".parse::<Dial>()?;
/// # Ok::<(), isahc::config::DialParseError>(())
/// ```
#[derive(Debug)]
pub struct Dial(Inner);

#[derive(Debug, Eq, PartialEq)]
enum Inner {
    Default,

    IpSocket(String),

    #[cfg(unix)]
    UnixSocket(std::path::PathBuf),
}

impl Dial {
    /// Connect to the given IP socket.
    ///
    /// Any value that can be converted into a [`SocketAddr`] can be given as an
    /// argument; check the [`SocketAddr`] documentation for a complete list.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::config::Dial;
    /// use std::net::Ipv4Addr;
    ///
    /// let dial = Dial::ip_socket((Ipv4Addr::LOCALHOST, 8080));
    /// ```
    ///
    /// ```
    /// use isahc::config::Dial;
    /// use std::net::SocketAddr;
    ///
    /// let dial = Dial::ip_socket("0.0.0.0:8765".parse::<SocketAddr>()?);
    /// # Ok::<(), std::net::AddrParseError>(())
    /// ```
    pub fn ip_socket(addr: impl Into<SocketAddr>) -> Self {
        // Create a string in the format CURLOPT_CONNECT_TO expects.
        Self(Inner::IpSocket(format!("::{}", addr.into())))
    }

    /// Connect to a Unix socket described by a file.
    ///
    /// The path given is not checked ahead of time for correctness or that the
    /// socket exists. If the socket is invalid an error will be returned when a
    /// request attempt is made.
    ///
    /// # Availability
    ///
    /// This function is only available on Unix.
    #[cfg(unix)]
    pub fn unix_socket(path: impl Into<std::path::PathBuf>) -> Self {
        Self(Inner::UnixSocket(path.into()))
    }
}

impl Default for Dial {
    fn default() -> Self {
        Self(Inner::Default)
    }
}

impl From<SocketAddr> for Dial {
    fn from(socket_addr: SocketAddr) -> Self {
        Self::ip_socket(socket_addr)
    }
}

impl FromStr for Dial {
    type Err = DialParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("tcp:") {
            let addr_str = s[4..].trim_start_matches("/");

            return addr_str
                .parse::<SocketAddr>()
                .map(Self::ip_socket)
                .map_err(|_| DialParseError(()));
        }

        #[cfg(unix)]
        if s.starts_with("unix:") {
            return Ok(Self::unix_socket(&s[5..]));
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
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        let mut connect_to = List::new();

        if let Inner::IpSocket(addr) = &self.0 {
            connect_to.append(addr)?;
        }

        easy.connect_to(connect_to)?;

        #[cfg(unix)]
        easy.unix_socket_path(match &self.0 {
            Inner::UnixSocket(path) => Some(path),
            _ => None,
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tcp_socket_and_port_uri() {
        let dial = "tcp:127.0.0.1:1200".parse::<Dial>().unwrap();

        assert_eq!(dial.0, Inner::IpSocket("::127.0.0.1:1200".into()));
    }

    #[test]
    fn parse_invalid_tcp_uri() {
        let result = "tcp:127.0.0.1-1200".parse::<Dial>();

        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn parse_unix_socket_uri() {
        let dial = "unix:/path/to/my.sock".parse::<Dial>().unwrap();

        assert_eq!(dial.0, Inner::UnixSocket("/path/to/my.sock".into()));
    }
}
