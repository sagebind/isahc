//! Request and response metrics tracking.

use crossbeam_utils::atomic::AtomicCell;
use std::{fmt, sync::Arc, time::Duration};

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

    // An overview of the six time values (taken from the curl documentation):
    //
    // curl_easy_perform()
    //     |
    //     |--NAMELOOKUP
    //     |--|--CONNECT
    //     |--|--|--APPCONNECT
    //     |--|--|--|--PRETRANSFER
    //     |--|--|--|--|--STARTTRANSFER
    //     |--|--|--|--|--|--TOTAL
    //     |--|--|--|--|--|--REDIRECT
    //
    // The numbers we expose in the API are a little more "high-level" than the
    // ones written here.
    pub(crate) namelookup_time: AtomicCell<f64>,
    pub(crate) connect_time: AtomicCell<f64>,
    pub(crate) appconnect_time: AtomicCell<f64>,
    pub(crate) pretransfer_time: AtomicCell<f64>,
    pub(crate) starttransfer_time: AtomicCell<f64>,
    pub(crate) total_time: AtomicCell<f64>,
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
    pub fn name_lookup_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.namelookup_time.load())
    }

    /// Get the amount of time taken to establish a connection to the server
    /// (not including TLS connection time).
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn connect_time(&self) -> Duration {
        Duration::from_secs_f64(
            (self.inner.connect_time.load() - self.inner.namelookup_time.load()).max(0f64),
        )
    }

    /// Get the amount of time spent on TLS handshakes.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn secure_connect_time(&self) -> Duration {
        let app_connect_time = self.inner.appconnect_time.load();

        if app_connect_time > 0f64 {
            Duration::from_secs_f64(app_connect_time - self.inner.connect_time.load())
        } else {
            Duration::new(0, 0)
        }
    }

    /// Get the time it took from the start of the request until the first
    /// byte is either sent or received.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn transfer_start_time(&self) -> Duration {
        Duration::from_secs_f64(self.inner.starttransfer_time.load())
    }

    /// Get the amount of time spent performing the actual request transfer. The
    /// "transfer" includes both sending the request and receiving the response.
    ///
    /// When a redirect is followed, the time from each request is added
    /// together.
    pub fn transfer_time(&self) -> Duration {
        Duration::from_secs_f64(
            (self.inner.total_time.load() - self.inner.starttransfer_time.load()).max(0f64),
        )
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
            .field("name_lookup_time", &self.name_lookup_time())
            .field("connect_time", &self.connect_time())
            .field("secure_connect_time", &self.secure_connect_time())
            .field("transfer_start_time", &self.transfer_start_time())
            .field("transfer_time", &self.transfer_time())
            .field("total_time", &self.total_time())
            .field("redirect_time", &self.redirect_time())
            .finish()
    }
}
