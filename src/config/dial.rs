//! Configuration for customizing how connections are established and sockets
//! are opened.

use super::SetOpt;
use curl::easy::{Easy2, List};
use http::Uri;
use std::{convert::TryFrom, fmt, net::SocketAddr, str::FromStr};

/// An error which can be returned when parsing a dial address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialerParseError(());

impl fmt::Display for DialerParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("invalid dial address syntax")
    }
}

impl std::error::Error for DialerParseError {}

/// A custom address or dialer for connecting to a host.
///
/// A dialer can be created from a URI-like string using [`FromStr`]. The
/// following URI schemes are supported:
///
/// - `tcp`: Connect to a TCP address and port pair, like `tcp:127.0.0.1:8080`.
/// - `unix`: Connect to a Unix socket located on the file system, like
///   `unix:/path/to/my.sock`. This is only supported on Unix.
///
/// The [`Default`] dialer uses the hostname and port specified in each request
/// as normal.
///
/// # Examples
///
/// Connect to a Unix socket URI:
///
/// ```
/// use isahc::config::Dialer;
///
/// # #[cfg(unix)]
/// let unix_socket = "unix:/path/to/my.sock".parse::<Dialer>()?;
/// # Ok::<(), isahc::config::DialerParseError>(())
/// ```
#[derive(Clone, Debug)]
pub struct Dialer(Inner);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Inner {
    Default,

    IpSocket(String),

    #[cfg(unix)]
    UnixSocket(std::path::PathBuf),
}

impl Dialer {
    /// Connect to the given IP socket.
    ///
    /// Any value that can be converted into a [`SocketAddr`] can be given as an
    /// argument; check the [`SocketAddr`] documentation for a complete list.
    ///
    /// # Examples
    ///
    /// ```
    /// use isahc::config::Dialer;
    /// use std::net::Ipv4Addr;
    ///
    /// let dialer = Dialer::ip_socket((Ipv4Addr::LOCALHOST, 8080));
    /// ```
    ///
    /// ```
    /// use isahc::config::Dialer;
    /// use std::net::SocketAddr;
    ///
    /// let dialer = Dialer::ip_socket("0.0.0.0:8765".parse::<SocketAddr>()?);
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
    /// # Examples
    ///
    /// ```
    /// use isahc::config::Dialer;
    ///
    /// let docker = Dialer::unix_socket("/var/run/docker.sock");
    /// ```
    ///
    /// # Availability
    ///
    /// This function is only available on Unix.
    #[cfg(unix)]
    pub fn unix_socket(path: impl Into<std::path::PathBuf>) -> Self {
        Self(Inner::UnixSocket(path.into()))
    }
}

impl Default for Dialer {
    fn default() -> Self {
        Self(Inner::Default)
    }
}

impl From<SocketAddr> for Dialer {
    fn from(socket_addr: SocketAddr) -> Self {
        Self::ip_socket(socket_addr)
    }
}

impl FromStr for Dialer {
    type Err = DialerParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("tcp:") {
            let addr_str = s[4..].trim_start_matches('/');

            return addr_str
                .parse::<SocketAddr>()
                .map(Self::ip_socket)
                .map_err(|_| DialerParseError(()));
        }

        #[cfg(unix)]
        {
            if s.starts_with("unix:") {
                // URI paths are always absolute.
                let mut path = std::path::PathBuf::from("/");
                path.push(&s[5..].trim_start_matches('/'));

                return Ok(Self(Inner::UnixSocket(path)));
            }
        }

        Err(DialerParseError(()))
    }
}

impl TryFrom<&'_ str> for Dialer {
    type Error = DialerParseError;

    fn try_from(str: &str) -> Result<Self, Self::Error> {
        str.parse()
    }
}

impl TryFrom<String> for Dialer {
    type Error = DialerParseError;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        string.parse()
    }
}

impl TryFrom<Uri> for Dialer {
    type Error = DialerParseError;

    fn try_from(uri: Uri) -> Result<Self, Self::Error> {
        // Not the most efficient implementation, but straightforward.
        uri.to_string().parse()
    }
}

impl SetOpt for Dialer {
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
        let dialer = "tcp:127.0.0.1:1200".parse::<Dialer>().unwrap();

        assert_eq!(dialer.0, Inner::IpSocket("::127.0.0.1:1200".into()));
    }

    #[test]
    fn parse_invalid_tcp_uri() {
        let result = "tcp:127.0.0.1-1200".parse::<Dialer>();

        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn parse_unix_socket_uri() {
        let dialer = "unix:/path/to/my.sock".parse::<Dialer>().unwrap();

        assert_eq!(dialer.0, Inner::UnixSocket("/path/to/my.sock".into()));
    }

    #[test]
    #[cfg(unix)]
    fn from_unix_socket_uri() {
        let uri = "unix://path/to/my.sock".parse::<http::Uri>().unwrap();
        let dialer = Dialer::try_from(uri).unwrap();

        assert_eq!(dialer.0, Inner::UnixSocket("/path/to/my.sock".into()));
    }
}
