use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(crate) struct Timer {
    timeout: Mutex<Option<Instant>>,
}

impl Timer {
    pub(crate) fn new() -> Self {
        Self {
            timeout: Mutex::new(None),
        }
    }

    pub(crate) fn is_expired(&self, now: Instant) -> bool {
        self.timeout
            .lock()
            .unwrap()
            .map(|timeout| now >= timeout)
            .unwrap_or(false)
    }

    pub(crate) fn get_remaining(&self, now: Instant) -> Option<Duration> {
        self.timeout
            .lock()
            .unwrap()
            .map(|timeout| timeout.saturating_duration_since(now))
    }

    pub(crate) fn start(&self, timeout: Duration) {
        *self.timeout.lock().unwrap() = Some(Instant::now() + timeout);
    }

    pub(crate) fn stop(&self) {
        *self.timeout.lock().unwrap() = None;
    }
}
