//! Inter-thread communication on the current (or final) state and statistics of
//! a particular request.

use http::Uri;
use lazycell::AtomicLazyCell;
use std::sync::{
    atomic::{
        AtomicU64,
        AtomicUsize,
        Ordering,
    },
    Arc,
};
use std::time::Duration;

/// An object that holds status updates and progress statistics on a particular
/// request. A [`Stat`] can be shared between threads, which allows an agent
/// thread to post updates to the object while consumers can read from the
/// object simultaneously.
///
/// Reading stats is not always guaranteed to be up-to-date.
#[derive(Clone, Debug, Default)]
pub(crate) struct Stat {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    effective_uri: AtomicLazyCell<Uri>,
    redirect_count: AtomicUsize,
    header_time: AtomicLazyCell<Duration>,
    upload_progress: AtomicU64,
    upload_total: AtomicU64,
    download_progress: AtomicU64,
    download_total: AtomicU64,
}

impl Stat {
    pub(crate) fn effective_uri(&self) -> Option<&Uri> {
        self.inner.effective_uri.borrow()
    }

    pub(crate) fn set_effective_uri(&self, uri: Uri) {
        self.inner.effective_uri.fill(uri).ok();
    }

    pub(crate) fn redirect_count(&self) -> usize {
        self.inner.redirect_count.load(Ordering::Relaxed)
    }

    pub(crate) fn set_redirect_count(&self, count: usize) {
        self.inner.redirect_count.store(count, Ordering::Relaxed);
    }

    pub(crate) fn header_time(&self) -> Option<Duration> {
        self.inner.header_time.get()
    }

    pub(crate) fn set_header_time(&self, time: Duration) {
        self.inner.header_time.fill(time).ok();
    }

    pub(crate) fn post_progress(
        &self,
        upload_progress: u64,
        upload_total: u64,
        download_progress: u64,
        download_total: u64,
    ) {
        self.inner.upload_progress.store(upload_progress, Ordering::Relaxed);
        self.inner.upload_total.store(upload_total, Ordering::Relaxed);
        self.inner.download_progress.store(download_progress, Ordering::Relaxed);
        self.inner.download_total.store(download_total, Ordering::Relaxed);
    }
}
