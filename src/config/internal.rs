//! Internal traits that define the Isahc configuration system.

use curl::easy::Easy2;

/// Base trait for any object that can be configured for requests, such as an
/// HTTP request builder or an HTTP client.
#[doc(hidden)]
pub trait ConfigurableBase: Sized {
    /// Configure this object with the given property, returning the configured
    /// self.
    #[doc(hidden)]
    fn configure(self, property: impl Send + Sync + 'static) -> Self;
}

/// A helper trait for applying a configuration value to a given curl handle.
pub(crate) trait SetOpt {
    /// Apply this configuration property to the given curl handle.
    #[doc(hidden)]
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error>;
}
