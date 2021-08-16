use super::request::SetOpt;
use std::time::Duration;

#[derive(Debug, Default)]
pub(crate) struct ClientConfig {
    pub(crate) connection_cache_ttl: Option<Duration>,
    pub(crate) close_connections: bool,
}

impl SetOpt for ClientConfig {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        if let Some(ttl) = self.connection_cache_ttl {
            easy.maxage_conn(ttl)?;
        }

        easy.forbid_reuse(self.close_connections)
    }
}
