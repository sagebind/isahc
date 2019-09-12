//! Inter-thread communication on the current (or final) state and statistics of
//! a particular request.

use std::sync::{
    atomic::{
        AtomicU64,
        AtomicUsize,
        Ordering,
    },
    Arc,
};

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
    upload_progress: AtomicU64,
    upload_total: AtomicU64,
    download_progress: AtomicU64,
    download_total: AtomicU64,
}

impl Stat {
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
