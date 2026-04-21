use crate::config::setopt::{SetOpt, SetOptError};
use curl::easy::Easy2;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ops::BitAnd,
};

#[cfg(unix)]
pub use unix::*;

/// Types implementing this trait can be used to select a network interface to
/// use for outgoing connections based on some criteria.
///
/// This trait is sealed and cannot be implemented outside of this module.
#[expect(private_bounds)]
pub trait InterfaceSelector: ToInterfaceString {}

/// Private version of the public trait. The public trait acts as just a marker,
/// while the methods can only be called in this module.
trait ToInterfaceString {
    fn to_interface_string(&self) -> InterfaceString;
}

impl<T: ToInterfaceString> InterfaceSelector for T {}

/// Use whatever interface the TCP stack finds suitable.
#[derive(Clone, Copy, Debug)]
pub struct AnyInterface;

impl ToInterfaceString for AnyInterface {
    fn to_interface_string(&self) -> InterfaceString {
        InterfaceString(None)
    }
}

impl ToInterfaceString for IpAddr {
    fn to_interface_string(&self) -> InterfaceString {
        InterfaceString(Some(format!("host!{}", self)))
    }
}

impl ToInterfaceString for Ipv4Addr {
    fn to_interface_string(&self) -> InterfaceString {
        IpAddr::from(*self).to_interface_string()
    }
}

impl ToInterfaceString for Ipv6Addr {
    fn to_interface_string(&self) -> InterfaceString {
        IpAddr::from(*self).to_interface_string()
    }
}

/// Selects a network interface to use for outgoing connections based on some
/// criteria.
#[derive(Clone, Debug)]
pub(crate) struct InterfaceString(Option<String>);

impl<T: InterfaceSelector> From<T> for InterfaceString {
    fn from(selector: T) -> Self {
        selector.to_interface_string()
    }
}

impl SetOpt for InterfaceString {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError> {
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

/// Unix-specific network interface selectors.
#[cfg(unix)]
mod unix {
    use super::*;
    use std::fmt;

    /// Selects a network interface based on its name (such as `eth0`). This
    /// selector is not available on Windows as it does not really have names
    /// for network devices.
    pub struct InterfaceName<Name: AsRef<str>>(pub Name);

    impl<Name: AsRef<str> + Clone> Clone for InterfaceName<Name> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<Name: AsRef<str>> fmt::Debug for InterfaceName<Name> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let name: &str = self.0.as_ref();

            f.debug_tuple("InterfaceName").field(&name).finish()
        }
    }

    impl<Name: AsRef<str>> ToInterfaceString for InterfaceName<Name> {
        fn to_interface_string(&self) -> InterfaceString {
            InterfaceString(Some(format!("if!{}", self.0.as_ref())))
        }
    }

    /// Selects a network interface to use for outgoing connections based on
    /// both its name and current IP address. Both criteria must be satisfied
    /// for the interface to be selected.
    #[derive(Clone, Debug)]
    pub struct InterfaceNameAndIpAddr<Name: AsRef<str>> {
        name: InterfaceName<Name>,
        ip_addr: IpAddr,
    }

    impl<Name: AsRef<str>> ToInterfaceString for InterfaceNameAndIpAddr<Name> {
        fn to_interface_string(&self) -> InterfaceString {
            InterfaceString(Some(format!(
                "ifhost!{}!{}",
                self.name.0.as_ref(),
                self.ip_addr
            )))
        }
    }

    impl<Name, Addr> BitAnd<Addr> for InterfaceName<Name>
    where
        Name: AsRef<str>,
        Addr: Into<IpAddr>,
    {
        type Output = InterfaceNameAndIpAddr<Name>;

        fn bitand(self, rhs: Addr) -> Self::Output {
            InterfaceNameAndIpAddr {
                name: self,
                ip_addr: rhs.into(),
            }
        }
    }

    impl<Name> BitAnd<InterfaceName<Name>> for IpAddr
    where
        Name: AsRef<str>,
    {
        type Output = InterfaceNameAndIpAddr<Name>;

        fn bitand(self, rhs: InterfaceName<Name>) -> Self::Output {
            InterfaceNameAndIpAddr {
                name: rhs,
                ip_addr: self,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_any_interface() {
        assert!(AnyInterface.to_interface_string().0.is_none());
    }

    #[test]
    fn test_interface_name() {
        let selector = InterfaceName("eth0");
        assert_eq!(selector.to_interface_string().0.unwrap(), "if!eth0");
    }

    #[test]
    fn test_interface_ip_addr() {
        let selector = Ipv4Addr::new(192, 168, 1, 1);
        assert_eq!(
            selector.to_interface_string().0.unwrap(),
            "host!192.168.1.1"
        );
    }

    #[test]
    fn test_interface_name_and_ip_addr() {
        let selector = InterfaceName("eth0") & Ipv4Addr::new(192, 168, 1, 1);
        assert_eq!(
            selector.to_interface_string().0.unwrap(),
            "ifhost!eth0!192.168.1.1"
        );
    }
}
