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
use once_cell::sync::Lazy;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use publicsuffix::{List, Psl};
use std::{error::Error, time::{Duration, SystemTime}};

/// How long should we use a cached list before refreshing?
static TTL: Lazy<Duration> = Lazy::new(|| Duration::from_secs(24 * 60 * 60));

/// Global in-memory PSL cache.
static CACHE: Lazy<RwLock<ListCache>> = Lazy::new(Default::default);

struct ListCache {
    list: List,
    last_refreshed: Option<SystemTime>,
    last_updated: Option<SystemTime>,
}

impl Default for ListCache {
    fn default() -> Self {
        Self {
            // Use a bundled version of the list. We bundle using a Git
            // submodule instead of downloading it from the Internet during the
            // build, because that would force you to have an active Internet
            // connection in order to compile. And that would be really
            // annoying, especially if you are on a slow connection.
            list: include_str!("list/public_suffix_list.dat")
                .parse()
                .expect("could not parse bundled public suffix list"),

            // Refresh the list right away.
            last_refreshed: None,

            // Assume the bundled list is always out of date.
            last_updated: None,
        }
    }
}

impl ListCache {
    fn needs_refreshed(&self) -> bool {
        match self.last_refreshed {
            Some(last_refreshed) => match last_refreshed.elapsed() {
                Ok(elapsed) => elapsed > *TTL,
                Err(_) => false,
            },
            None => true,
        }
    }

    fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
        let result = self.try_refresh();
        self.last_refreshed = Some(SystemTime::now());
        result
    }

    fn try_refresh(&mut self) -> Result<(), Box<dyn Error>> {
        let mut request = http::Request::get(publicsuffix::LIST_URL);

        if let Some(last_updated) = self.last_updated {
            request = request.header(http::header::IF_MODIFIED_SINCE, httpdate::fmt_http_date(last_updated));
        }

        let mut response = request.body(())?.send()?;

        match response.status() {
            http::StatusCode::OK => {
                // Parse the suffix list.
                self.list = response.text()?.parse()?;
                self.last_updated = Some(SystemTime::now());
                tracing::debug!("public suffix list updated");
            }

            http::StatusCode::NOT_MODIFIED => {
                // List hasn't changed and is still new.
                self.last_updated = Some(SystemTime::now());
            }

            status => tracing::warn!(
                "could not update public suffix list, got status code {}",
                status,
            ),
        }

        Ok(())
    }
}

/// Determine if the given domain is a public suffix.
///
/// If the current list information is stale, a background refresh will be
/// triggered. The current data will be used to respond to this query.
pub(crate) fn is_public_suffix(domain: impl AsRef<str>) -> bool {
    let domain = domain.as_ref().as_bytes();

    with_cache(|cache| {
        // Check if the given domain is a public suffix.
        cache
            .list
            .suffix(domain)
            // We don't want to block unknown hosts like `localhost`
            .filter(publicsuffix::Suffix::is_known)
            .filter(|suffix| suffix == &domain)
            .is_some()
    })
}

/// Execute a given closure with a reference to the list cache. If the list is
/// out of date, attempt to refresh it first before continuing.
fn with_cache<T>(f: impl FnOnce(&ListCache) -> T) -> T {
    let cache = CACHE.upgradable_read();

    // First check if the list needs to be refreshed.
    if cache.needs_refreshed() {
        // Upgrade our lock to gain write access.
        let mut cache = RwLockUpgradableReadGuard::upgrade(cache);

        // If there was contention then the cache might not need refreshed any
        // more.
        if cache.needs_refreshed() {
            if let Err(e) = cache.refresh() {
                tracing::warn!("could not refresh public suffix list: {}", e);
            }
        }

        f(&*cache)
    } else {
        f(&*cache)
    }
}
