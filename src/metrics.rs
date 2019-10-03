//! Request and response metrics tracking.

use crossbeam_utils::atomic::AtomicCell;
use std::{
    fmt,
    sync::Arc,
    time::Duration,
};

/// An object that holds status updates and progress statistics on a particular
/// request. A [`Metrics`] can be shared between threads, which allows an agent
/// thread to post updates to the object while consumers can read from the
/// object simultaneously.
///
/// Reading stats is not always guaranteed to be up-to-date.
#[derive(Clone)]
pub struct Metrics {
    pub(crate) inner: Arc<Inner>,
}

#[derive(Default)]
pub(crate) struct Inner {
    pub(crate) upload_progress: AtomicCell<f64>,
    pub(crate) upload_total: AtomicCell<f64>,
    pub(crate) download_progress: AtomicCell<f64>,
    pub(crate) download_total: AtomicCell<f64>,
    pub(crate) upload_speed: AtomicCell<f64>,
    pub(crate) download_speed: AtomicCell<f64>,
    pub(crate) total_time: AtomicCell<f64>,
    pub(crate) namelookup_time: AtomicCell<f64>,
    pub(crate) connect_time: AtomicCell<f64>,
    pub(crate) appconnect_time: AtomicCell<f64>,
    pub(crate) pretransfer_time: AtomicCell<f64>,
    pub(crate) starttransfer_time: AtomicCell<f64>,
    pub(crate) redirect_time: AtomicCell<f64>,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::default(),
        }
    }

    /// Number of bytes uploaded / estimated total.
    pub fn upload_progress(&self) -> (u64, u64) {
        (
            self.inner.upload_progress.load() as u64,
            self.inner.upload_total.load() as u64,
        )
    }

    /// Average upload speed so far in bytes/second.
    pub fn upload_speed(&self) -> f64 {
        self.inner.upload_speed.load()
    }

    /// Number of bytes downloaded / estimated total.
    pub fn download_progress(&self) -> (u64, u64) {
        (
            self.inner.download_progress.load() as u64,
            self.inner.download_total.load() as u64,
        )
    }

    /// Average download speed so far in bytes/second.
    pub fn download_speed(&self) -> f64 {
        self.inner.download_speed.load()
    }

    /// Get the total time from the start of the request until DNS name
    /// resolving was completed.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn namelookup_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.namelookup_time.load())
    }

    /// Get the total time from the start of the request until the connection to
    /// the remote host (or proxy) was completed.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn connect_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.connect_time.load())
    }

    /// Get the time it took from the start of the request until the SSL/SSH
    /// connect/handshake to the remote host was completed.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn appconnect_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.appconnect_time.load())
    }

    /// Get the time it took from the start of the request until the file
    /// transfer is just about to begin. This includes all pre-transfer commands
    /// and negotiations that are specific to the particular protocol(s)
    /// involved. It does not involve the sending of the protocol-specific
    /// request that triggers a transfer.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn pretransfer_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.pretransfer_time.load())
    }

    /// Get the time it took from the start of the request until the first
    /// response byte is received.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn starttransfer_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.starttransfer_time.load())
    }

    /// Get the total time for the entire request. This will continuously
    /// increase until the entire response body is consumed and completed.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn total_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.total_time.load())
    }

    /// If automatic redirect following is enabled, gets the total time taken
    /// for all redirection steps including name lookup, connect, pretransfer
    /// and transfer before final transaction was started.
    pub fn redirect_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.redirect_time.load())
    }
}

impl fmt::Debug for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metrics")
            .field("upload_progress", &self.upload_progress())
            .field("upload_speed", &self.upload_speed())
            .field("download_progress", &self.download_progress())
            .field("download_speed", &self.download_speed())
            .field("namelookup_time", &self.namelookup_time())
            .field("connect_time", &self.connect_time())
            .field("appconnect_time", &self.appconnect_time())
            .field("pretransfer_time", &self.pretransfer_time())
            .field("starttransfer_time", &self.starttransfer_time())
            .field("total_time", &self.total_time())
            .field("redirect_time", &self.redirect_time())
            .finish()
    }
}

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Upload")?;
        writeln!(f, "{}/{}", self.upload_progress().0, self.upload_progress().1)
    }
}
