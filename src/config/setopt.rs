use curl::easy::Easy2;

/// A helper trait for applying a configuration value to a given curl handle.
pub(crate) trait SetOpt {
    /// Apply this configuration property to the given curl handle.
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError>;
}

/// Like [`SetOpt`], but applies the configuration specifically for proxy
/// connections rather than the origin itself.
pub(crate) trait SetOptProxy: SetOpt {
    fn set_opt_proxy<H>(&self, easy: &mut Easy2<H>) -> Result<(), SetOptError>;
}

pub(crate) enum SetOptError {
    Curl(curl::Error),
    Other(crate::error::Error),
}

impl From<curl::Error> for SetOptError {
    fn from(err: curl::Error) -> Self {
        SetOptError::Curl(err)
    }
}

impl From<crate::error::Error> for SetOptError {
    fn from(err: crate::error::Error) -> Self {
        SetOptError::Other(err)
    }
}

impl From<SetOptError> for crate::error::Error {
    fn from(err: SetOptError) -> Self {
        match err {
            SetOptError::Curl(err) => crate::error::Error::from_any(err),
            SetOptError::Other(err) => err,
        }
    }
}
