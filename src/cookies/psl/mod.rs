//! This module provides access to the [Public Suffix
//! List](https://publicsuffix.org), a community-supported database of domain
//! "public suffixes". This list is commonly used by web browsers and HTTP
//! clients to prevent cookies from being set for a high-level domain name
//! suffix, which could be exploited maliciously.
//!
//! Ideally, clients should use a recent copy of the list in cookie validation.
//! Applications such as web browsers tend to be on a frequent update cycle, and
//! so they usually download a local copy of the list at compile time and use
//! that until the next build. Since HTTP clients tend to be used in a much
//! different way and are often embedded into long-lived software without
//! frequent (or any) updates, it is better for us to download a fresh copy from
//! the Internet every once in a while to make sure the list isn't too stale.
//!
//! Despite being in an HTTP client, we can't always assume that the Internet is
//! available (we might be behind a firewall or offline), we _also_ include an
//! offline copy of the list, which is embedded here at compile time. If the
//! embedded list is stale, then we attempt to download a newer copy of the
//! list. If we can't, then we log a warning and use the stale list anyway,
//! since a stale list is better than no list at all.

use crate::{request::RequestExt, ReadResponseExt};
use crossbeam_utils::atomic::AtomicCell;
use once_cell::sync::Lazy;
use publicsuffix::{List, Psl};
use std::{
    error::Error,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, SystemTime},
};

/// How long should we use a cached list before refreshing?
static TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Global in-memory PSL cache.
static CACHE: Lazy<ListCache> = Lazy::new(Default::default);

#[derive(Clone)]
struct ListCache {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    list: List,
    last_refreshed: Option<SystemTime>,
    last_modified: Option<SystemTime>,
    is_refreshing: AtomicCell<bool>,
}

impl Default for ListCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                // Use a bundled version of the list. We bundle using a Git
                // submodule instead of downloading it from the Internet during the
                // build, because that would force you to have an active Internet
                // connection in order to compile. And that would be really
                // annoying, especially if you are on a slow connection.
                list: include_str!("list/public_suffix_list.dat")
                    .parse()
                    .expect("could not parse bundled public suffix list"),

                // Refresh the list right away.
                last_refreshed: Default::default(),

                // Assume the bundled list is always out of date.
                last_modified: Default::default(),

                is_refreshing: Default::default(),
            })),
        }
    }
}

impl ListCache {
    // Check if the given domain is a public suffix.
    fn is_public_suffix(&self, domain: &str) -> bool {
        let domain = domain.as_bytes();

        self.inner
            .read()
            .unwrap()
            .list
            .suffix(domain)
            // We don't want to block unknown hosts like `localhost`
            .filter(publicsuffix::Suffix::is_known)
            .filter(|suffix| suffix == &domain)
            .is_some()
    }

    fn refresh(&self) -> Result<(), Box<dyn Error>> {
        let mut inner = self.inner.write().unwrap();
        let mut request = http::Request::get(publicsuffix::LIST_URL);

        if let Some(last_modified) = inner.last_modified {
            request = request.header(
                http::header::IF_MODIFIED_SINCE,
                httpdate::fmt_http_date(last_modified),
            );
        }

        let mut response = request.body(())?.send()?;

        match response.status() {
            http::StatusCode::OK => {
                // Parse the suffix list.
                inner.list = response.text()?.parse()?;
                tracing::debug!("public suffix list updated");
            }

            http::StatusCode::NOT_MODIFIED => {
                tracing::debug!("public suffix list not modified");
            }

            status => {
                tracing::warn!(
                    "could not update public suffix list, got status code {}",
                    status,
                );
                return Ok(());
            }
        }

        if let Some(d) = response.headers().get(http::header::LAST_MODIFIED) {
            inner.last_modified = httpdate::parse_http_date(d.to_str().unwrap()).ok();
        }

        inner.last_refreshed = Some(SystemTime::now());

        Ok(())
    }

    fn refresh_in_background(&self, force: bool) {
        let inner = self.inner.read().unwrap();

        if !force && !inner.needs_refreshed() {
            return;
        }

        // Only spawn a refresh thread if one isn't already running.
        if inner.is_refreshing.compare_exchange(false, true).is_ok() {
            let cache = self.clone();

            thread::spawn(move || {
                if let Err(e) = cache.refresh() {
                    tracing::warn!("could not refresh public suffix list: {}", e);
                }

                cache.inner.read().unwrap().is_refreshing.store(false);
            });
        }
    }
}

impl Inner {
    fn needs_refreshed(&self) -> bool {
        match self.last_refreshed {
            Some(last_refreshed) => match last_refreshed.elapsed() {
                Ok(elapsed) => elapsed > TTL,
                Err(_) => false,
            },
            None => true,
        }
    }
}

/// Determine if the given domain is a public suffix.
///
/// If the current list information is stale, a background refresh will be
/// triggered. The current data will be used to respond to this query.
pub(crate) fn is_public_suffix(domain: impl AsRef<str>) -> bool {
    let domain = domain.as_ref();

    // Refresh the list if needed.
    CACHE.refresh_in_background(false);

    CACHE.is_public_suffix(domain)
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    #[test]
    fn refresh_cache() {
        let cache = ListCache::default();

        assert!(cache.inner.read().unwrap().last_refreshed.is_none());
        assert!(cache.inner.read().unwrap().last_modified.is_none());
        assert!(cache.inner.read().unwrap().needs_refreshed());

        cache.refresh_in_background(true);

        while cache.inner.read().unwrap().is_refreshing.load() {
            sleep(Duration::from_millis(100));
        }

        assert!(cache.inner.read().unwrap().last_refreshed.is_some());
        assert!(cache.inner.read().unwrap().last_modified.is_some());
        assert!(!cache.inner.read().unwrap().needs_refreshed());

        let last_refreshed = cache.inner.read().unwrap().last_refreshed.unwrap();
        let last_modified = cache.inner.read().unwrap().last_modified.unwrap();

        cache.refresh_in_background(true);

        while cache.inner.read().unwrap().is_refreshing.load() {
            sleep(Duration::from_millis(100));
        }

        assert!(cache.inner.read().unwrap().last_refreshed.unwrap() > last_refreshed);
        assert!(cache.inner.read().unwrap().last_modified.unwrap() >= last_modified);
    }
}
