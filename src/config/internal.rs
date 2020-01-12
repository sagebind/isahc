//! Internal traits that define the Isahc configuration system.

use curl::easy::Easy2;

/// Base trait for any object that can be configured for requests, such as an
/// HTTP request builder or an HTTP client.
#[doc(hidden)]
pub trait ConfigurableBase: Sized {
    /// Configure this object with the given property, returning the configured
    /// self.
    #[doc(hidden)]
    fn configure(self, property: impl SetOpt) -> Self;
}

/// A helper trait for applying a configuration value to a given curl handle.
#[doc(hidden)]
pub trait SetOpt: Send + Sync + 'static {
    /// Apply this configuration property to the given curl handle.
    #[doc(hidden)]
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error>;
}
