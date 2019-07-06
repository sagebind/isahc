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
use chrono::prelude::*;
use chrono::Duration;
use lazy_static::lazy_static;
use publicsuffix::List;
use std::error::Error;
use std::sync::RwLock;
use std::thread;

/// This is a bundled version of the list. We bundle using a Git submodule
/// instead of downloading it from the Internet during the build, because that
/// would force you to have an active Internet connection in order to compile.
/// And that would be really annoying, especially if you are on a slow
/// connection.
static BUNDLED_LIST: &str = include_str!("list/public_suffix_list.dat");

lazy_static! {
    /// How long should we use a cached list before refreshing?
    static ref TTL: Duration = Duration::hours(24);

    // Use a read/write lock because we are reading only 99.99% of the time.
    static ref LIST: RwLock<(List, Option<DateTime<Utc>>)> = RwLock::new((
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
    let (ref list, ref last_updated) = *LIST.read().unwrap();

    // First check if the list needs to be refreshed.
    let needs_refreshed = match last_updated {
        Some(last_updated) => Utc::now() - *last_updated > *TTL,
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
    let (ref mut list, ref mut last_updated) = *LIST.write().unwrap();

    let mut request = http::Request::get(publicsuffix::LIST_URL);

    if let Some(last_updated) = last_updated {
        request.header(http::header::IF_MODIFIED_SINCE, last_updated.to_rfc2822());
    }

    let mut response = request.body(())?.send()?;

    match response.status() {
        http::StatusCode::OK => {
            // Parse the suffix list.
            *list = List::from_reader(response.body_mut())?;
            *last_updated = Some(Utc::now());
            log::debug!("public suffix list updated");
        }

        http::StatusCode::NOT_MODIFIED => {
            // List hasn't changed, check again after TTL.
            *last_updated = Some(Utc::now());
        }

        status => {
            log::warn!("could not update public suffix list, got status code {}", status);
        }
    }

    Ok(())
}
