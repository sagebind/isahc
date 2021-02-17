use std::time::{Duration, Instant};

pub(crate) struct Timer {
    expires: Option<Instant>,
}

impl Timer {
    pub(crate) const fn new() -> Self {
        Self {
            expires: None,
        }
    }

    pub(crate) fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires {
            Instant::now() >= expires
        } else {
            false
        }
    }

    pub(crate) fn time_remaining(&self) -> Option<Duration> {
        if let Some(expires) = self.expires {
            expires.checked_duration_since(Instant::now())
        } else {
            None
        }
    }

    pub(crate) fn arm(&mut self, duration: Duration) {
        self.expires = Some(Instant::now() + duration);
        eprintln!("timer set: {:?}", duration);
    }

    pub(crate) fn clear(&mut self) {
        self.expires = None;
        eprintln!("timer cleared!");
    }
}
