use std::time::{Duration, Instant};

use crossbeam_utils::atomic::AtomicCell;

#[derive(Debug)]
pub(crate) struct Timer {
    timeout: AtomicCell<Option<Instant>>,
}

impl Timer {
    pub(crate) fn new() -> Self {
        Self {
            timeout: AtomicCell::new(None),
        }
    }

    pub(crate) fn is_expired(&self, now: Instant) -> bool {
        self.timeout
            .load()
            .map(|timeout| now >= timeout)
            .unwrap_or(false)
    }

    pub(crate) fn get_remaining(&self, now: Instant) -> Option<Duration> {
        self.timeout
            .load()
            .map(|timeout| timeout.saturating_duration_since(now))
    }

    pub(crate) fn start(&self, timeout: Duration) {
        self.timeout.store(Some(Instant::now() + timeout));
    }

    pub(crate) fn stop(&self) {
        self.timeout.store(None);
    }
}
