//! Internal extension methods for curl types.
//!
//! These should probably be upstreamed eventually.

#![allow(unsafe_code)]

use curl::{Error, easy::Easy2};
use std::{
    ffi::CString,
    path::Path,
    ptr,
};

pub(crate) trait EasyExt {
    /// Set Unix socket path.
    ///
    /// Alternative to `Easy2::unix_socket` that does not require UTF-8 and
    /// allows unsetting.
    #[cfg(unix)]
    fn unix_socket_path(&mut self, path: Option<&Path>) -> Result<(), Error>;

    #[cfg(feature = "unstable-dial-ip")]
    fn connect_to(&mut self, connect_to: Option<&str>) -> Result<(), Error>;
}

impl<H> EasyExt for Easy2<H> {
    #[cfg(unix)]
    fn unix_socket_path(&mut self, path: Option<&Path>) -> Result<(), Error> {
        let path = if let Some(path) = path {
            Some(path_to_cstring(path)?)
        } else {
            None
        };

        let ptr = path.as_ref()
            .map(|cstring| cstring.as_ptr())
            .unwrap_or(ptr::null());

        unsafe {
            match curl_sys::curl_easy_setopt(self.raw(), curl_sys::CURLOPT_UNIX_SOCKET_PATH, ptr) {
                curl_sys::CURLE_OK => Ok(()),
                code => Err(Error::new(code)),
            }
        }
    }

    #[cfg(feature = "unstable-dial-ip")]
    fn connect_to(&mut self, connect_to: Option<&str>) -> Result<(), Error> {
        let connect_to = if let Some(s) = connect_to {
            Some(CString::new(s)?)
        } else {
            None
        };

        unsafe {
            let slist = match connect_to {
                // TODO: This leaks.
                Some(s) => curl_sys::curl_slist_append(std::ptr::null_mut(), s.as_ptr()),
                None => ptr::null_mut(),
            };

            match curl_sys::curl_easy_setopt(self.raw(), 243, slist) {
                curl_sys::CURLE_OK => Ok(()),
                code => Err(curl::Error::new(code)),
            }
        }
    }
}

#[cfg(unix)]
fn path_to_cstring<P: AsRef<Path>>(path: P) -> Result<CString, Error> {
    use std::os::unix::ffi::OsStrExt;

    Ok(CString::new(path.as_ref().as_os_str().as_bytes().to_vec())?)
}

#[cfg(not(unix))]
fn path_to_cstring<P: AsRef<Path>>(path: P) -> Result<CString, Error> {
    match val.to_str() {
        Some(s) => Ok(CString::new(s)?),
        None => Err(Error::new(curl_sys::CURLE_CONV_FAILED)),
    }
}
