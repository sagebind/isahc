//! Runtime support for checking versions and feature availability.

use std::sync::LazyLock;

/// Gets a human-readable string with the version number of Isahc and its
/// dependencies.
///
/// This function can be helpful when troubleshooting issues in Isahc or one of
/// its dependencies.
pub fn version() -> &'static str {
    static VERSION_STRING: LazyLock<String> = LazyLock::new(|| {
        format!(
            "isahc/{} (features:{}) {}",
            env!("CARGO_PKG_VERSION"),
            env!("ISAHC_FEATURES"),
            curl::Version::num(),
        )
    });

    &VERSION_STRING
}

/// Check if runtime support is available for the given HTTP version.
///
/// This only indicates whether support for communicating with this HTTP version
/// is available, which is usually determined by which features were enabled
/// during compilation, but can also be affected by what is available in system
/// libraries when using dynamic linking.
///
/// This does not indicate which versions Isahc will attempt to use by default.
/// To customize which versions to use within a particular client or request
/// instance, see [`VersionNegotiation`][crate::config::VersionNegotiation].
pub fn is_http_version_supported(version: http::Version) -> bool {
    match version {
        // HTTP/0.9 was disabled by default as of 7.66.0. See also
        // https://github.com/sagebind/isahc/issues/310 if we ever decide to
        // allow enabling it again.
        http::Version::HTTP_09 => curl_version() < (7, 66, 0),
        http::Version::HTTP_10 => true,
        http::Version::HTTP_11 => true,
        http::Version::HTTP_2 => curl_info().feature_http2(),
        http::Version::HTTP_3 => curl_info().feature_http3(),
        _ => false,
    }
}

/// Get version and runtime information about the instance of curl Isahc is
/// linked to.
#[inline]
pub(crate) fn curl_info() -> &'static curl::Version {
    // Query for curl version info just once since it is immutable.
    static CURL_VERSION: LazyLock<curl::Version> = LazyLock::new(curl::Version::get);

    &CURL_VERSION
}

/// Get the major, minor, and patch version numbers of the instance of curl
/// Isahc is linked to. Regardless of how Isahc is compiled, this will return
/// the actual library version being used.
pub(crate) fn curl_version() -> (u8, u8, u8) {
    let bits = curl_info().version_num();

    ((bits >> 16) as u8, (bits >> 8) as u8, bits as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_expected() {
        let version = version();

        assert!(version.starts_with("isahc/1."));
        assert!(version.contains("curl/7.") || version.contains("curl/8."));
    }

    #[test]
    fn curl_version_expected() {
        let (major, minor, _patch) = curl_version();

        assert!(major >= 7);
        assert!(minor > 0);
    }

    #[test]
    fn http1_always_supported() {
        assert!(is_http_version_supported(http::Version::HTTP_10));
        assert!(is_http_version_supported(http::Version::HTTP_11));

        if cfg!(feature = "http2") {
            assert!(is_http_version_supported(http::Version::HTTP_2));
        }
    }
}
