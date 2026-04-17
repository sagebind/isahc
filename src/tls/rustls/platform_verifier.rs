use crate::config::setopt::{SetOpt, SetOptError, SetOptProxy};
use curl::easy::SslOpt;

pub(crate) struct PlatformVerifierRoots;

impl SetOpt for PlatformVerifierRoots {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), SetOptError> {
        easy.ssl_options(SslOpt::new().native_ca(true))?;

        Ok(())
    }
}

impl SetOptProxy for PlatformVerifierRoots {
    fn set_opt_proxy<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), SetOptError> {
        easy.proxy_ssl_options(SslOpt::new().native_ca(true))?;

        Ok(())
    }
}
