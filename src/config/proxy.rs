use super::SetOpt;
use curl::easy::Easy2;
use std::iter::FromIterator;

/// Decorator for marking certain configurations to apply to a proxy rather than
/// the origin itself.
#[derive(Clone, Debug)]
pub(crate) struct Proxy<T>(pub(crate) T);

/// Proxy URI specifies the type and host of a proxy to use.
impl SetOpt for Proxy<Option<http::Uri>> {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        match &self.0 {
            Some(uri) => easy.proxy(&format!("{}", uri)),
            None => easy.proxy(""),
        }
    }
}

/// A list of host names that do not require a proxy to get reached, even if one
/// is specified.
///
/// See
/// [`HttpClientBuilder::proxy_blacklist`](crate::HttpClientBuilder::proxy_blacklist)
/// for configuring a client's no proxy list.
#[derive(Clone, Debug)]
pub(crate) struct Blacklist {
    skip: String,
}

impl FromIterator<String> for Blacklist {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self {
            skip: iter.into_iter().collect::<Vec<_>>().join(","),
        }
    }
}

impl SetOpt for Blacklist {
    fn set_opt<H>(&self, easy: &mut Easy2<H>) -> Result<(), curl::Error> {
        easy.noproxy(&self.skip)
    }
}
