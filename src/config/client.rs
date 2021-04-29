use super::{
    dns::{DnsCache, ResolveMap},
    request::SetOpt,
};
use std::time::Duration;

#[derive(Debug, Default)]
pub(crate) struct ClientConfig {
    pub(crate) connection_cache_ttl: Option<Duration>,
    pub(crate) close_connections: bool,
    pub(crate) dns_cache: Option<DnsCache>,
    pub(crate) dns_resolve: Option<ResolveMap>,
}

impl SetOpt for ClientConfig {
    fn set_opt<H>(&self, easy: &mut curl::easy::Easy2<H>) -> Result<(), curl::Error> {
        if let Some(ttl) = self.connection_cache_ttl {
            easy.maxage_conn(ttl)?;
        }

        if let Some(cache) = self.dns_cache.as_ref() {
            cache.set_opt(easy)?;
        }

        if let Some(map) = self.dns_resolve.as_ref() {
            map.set_opt(easy)?;
        }

        easy.forbid_reuse(self.close_connections)
    }
}
