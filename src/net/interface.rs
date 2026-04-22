//! Network interface selection for outgoing connections.

#![expect(private_interfaces)]

use crate::config::setopt::{EasyHandle, SetOpt, SetOptError};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[cfg(unix)]
pub use unix::*;

/// Types implementing this trait can be used as a condition to select a network
/// interface to use for outgoing connections based on some criteria.
///
/// You can combine multiple selectors in some cases using tuples to apply
/// multiple selectors, such as selecting an interface by both its name and IP
/// address. All selectors in the tuple must match the interface to be selected.
///
/// This trait is sealed and cannot be implemented outside of this crate.
pub trait Selector {
    #[doc(hidden)]
    fn into_interface_string(self, _: Sealed) -> InterfaceString;
}

/// Use whatever interface the TCP stack finds suitable.
#[derive(Clone, Copy, Debug, Default)]
pub struct Any;

impl Selector for Any {
    fn into_interface_string(self, _: Sealed) -> InterfaceString {
        InterfaceString(None)
    }
}

/// Bind to an interface by IP address.
impl Selector for IpAddr {
    fn into_interface_string(self, _: Sealed) -> InterfaceString {
        InterfaceString(Some(format!("host!{}", self)))
    }
}

/// Bind to an interface by IPv4 address.
impl Selector for Ipv4Addr {
    fn into_interface_string(self, _: Sealed) -> InterfaceString {
        IpAddr::from(self).into_interface_string(Sealed)
    }
}

/// Bind to an interface by IPv6 address.
impl Selector for Ipv6Addr {
    fn into_interface_string(self, _: Sealed) -> InterfaceString {
        IpAddr::from(self).into_interface_string(Sealed)
    }
}

/// Selects a network interface to use for outgoing connections based on some
/// criteria.
#[derive(Clone, Debug)]
pub(crate) struct InterfaceString(Option<String>);

impl<T: Selector> From<T> for InterfaceString {
    fn from(selector: T) -> Self {
        selector.into_interface_string(Sealed)
    }
}

impl SetOpt for InterfaceString {
    fn set_opt(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        match self.0.as_ref() {
            Some(interface) => easy.interface(interface).map_err(Into::into),

            // Use raw FFI because safe wrapper doesn't let us set to null.
            None => unsafe {
                match curl_sys::curl_easy_setopt(easy.raw(), curl_sys::CURLOPT_INTERFACE, 0) {
                    curl_sys::CURLE_OK => Ok(()),
                    code => Err(curl::Error::new(code).into()),
                }
            },
        }
    }
}

/// Private marker to seal the `Selector` trait methods and prevent external
/// implementations.
struct Sealed;

/// Unix-specific network interface selectors.
#[cfg(unix)]
mod unix {
    use super::*;
    use std::fmt;

    /// Selects a network interface based on its name (such as `eth0`). This
    /// selector is not available on Windows as it does not really have names
    /// for network devices.
    ///
    /// # Examples
    ///
    /// ```
    /// # use isahc::net::interface::Name;
    /// let loopback = Name("lo");
    /// let wifi = Name("wlan0");
    /// ```
    pub struct Name<T: AsRef<str>>(pub T);

    impl<T: AsRef<str> + Clone> Clone for Name<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<T: AsRef<str>> fmt::Debug for Name<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let name: &str = self.0.as_ref();

            f.debug_tuple("InterfaceName").field(&name).finish()
        }
    }

    impl<T: AsRef<str>> Selector for Name<T> {
        fn into_interface_string(self, _: Sealed) -> InterfaceString {
            InterfaceString(Some(format!("if!{}", self.0.as_ref())))
        }
    }

    /// Selects a network interface to use for outgoing connections based on
    /// both its name and current IP address. Both criteria must be satisfied
    /// for the interface to be selected.
    impl<T, U> Selector for (Name<T>, U)
    where
        T: AsRef<str>,
        U: Into<IpAddr> + Selector,
    {
        fn into_interface_string(self, _: Sealed) -> InterfaceString {
            InterfaceString(Some(format!(
                "ifhost!{}!{}",
                self.0.0.as_ref(),
                self.1.into()
            )))
        }
    }

    /// Selects a network interface to use for outgoing connections based on
    /// both its name and current IP address. Both criteria must be satisfied
    /// for the interface to be selected.
    impl<T, U> Selector for (T, Name<U>)
    where
        T: Into<IpAddr> + Selector,
        U: AsRef<str>,
    {
        fn into_interface_string(self, _: Sealed) -> InterfaceString {
            (self.1, self.0).into_interface_string(Sealed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_any_interface() {
        assert!(Any.into_interface_string(Sealed).0.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_interface_name() {
        let selector = Name("eth0");
        assert_eq!(selector.into_interface_string(Sealed).0.unwrap(), "if!eth0");
    }

    #[test]
    fn test_interface_ip_addr() {
        let selector = Ipv4Addr::new(192, 168, 1, 1);
        assert_eq!(
            selector.into_interface_string(Sealed).0.unwrap(),
            "host!192.168.1.1"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_interface_name_and_ip_addr() {
        let selector = (Name("eth0"), Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(
            selector.into_interface_string(Sealed).0.unwrap(),
            "ifhost!eth0!192.168.1.1"
        );
    }
}
