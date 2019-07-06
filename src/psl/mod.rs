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

use crate::request::RequestExt;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use publicsuffix::List;
use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};

/// This is a bundled version of the list. We bundle using a Git submodule
/// instead of downloading it from the Internet during the build, because that
/// would force you to have an active Internet connection in order to compile.
/// And that would be really annoying, especially if you are on a slow
/// connection.
static BUNDLED_LIST: &str = include_str!("list/public_suffix_list.dat");

/// How long should we use a cached list before refreshing?
static TTL: Duration = Duration::from_secs(60 * 60 * 24); // 24 hours

lazy_static! {
    // Use a read/write lock because we are reading only 99.99% of the time.
    static ref LIST: RwLock<(List, Option<Instant>)> = RwLock::new((
        List::from_str(BUNDLED_LIST).expect("could not parse bundled public suffix list"),
        // Assume the bundled list is always out of date.
        None,
    ));
}

/// Determine if the given domain is a public suffix.
///
/// If the current list information is stale, a background refresh will be
/// triggered. The current data will be used to respond to this query.
pub fn is_public_suffix(domain: impl AsRef<str>) -> bool {
    let domain = domain.as_ref();
    let (ref list, ref refreshed) = *LIST.read();

    // First check if the list needs to be refreshed.
    let needs_refreshed = match refreshed {
        Some(refreshed) => refreshed.elapsed() > TTL,
        None => true,
    };

    // TODO: Prevent multiple threads spawning at a time.
    // TODO: Retry timeout if refreshing fails.
    if needs_refreshed {
        // Refresh the list in a background thread.
        if let Err(e) = thread::Builder::new().name("psl-refresh".into()).spawn(|| {
            if let Err(e) = refresh() {
                log::warn!("could not refresh public suffix list: {}", e);
            }
        }) {
            log::error!("error refreshing public suffix list: {}", e);
        }
    }

    // Check if the given domain is a public suffix.
    list.parse_domain(domain)
        .ok()
        .and_then(|d| d.suffix().map(|d| d == domain))
        .unwrap_or(false)
}

/// Refresh the cached Public Suffix List synchronously. A new suffix list will
/// be downloaded from the official location at
/// <https://publicsuffix.org/list/public_suffix_list.dat>.
pub fn refresh() -> Result<(), Box<Error>> {
    // Yay, dogfooding!
    let mut response = http::Request::get(publicsuffix::LIST_URL)
        .header(http::header::IF_MODIFIED_SINCE, "value: V")
        .body(())?
        .send()?;

    // Parse the suffix list.
    let list = List::from_reader(response.body_mut())?;

    // Update the global cache.
    let mut lock = LIST.write();
    lock.0 = list;
    lock.1 = Some(Instant::now());
    drop(lock);

    log::debug!("public suffix list refreshed");

    Ok(())
}
