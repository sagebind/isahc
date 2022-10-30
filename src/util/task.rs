//! Helpers for working with tasks.

use std::task::Waker;
use waker_fn::waker_fn;

/// Helper methods for working with wakers.
pub(crate) trait WakerExt {
    /// Create a new waker from a closure that accepts this waker as an
    /// argument.
    fn chain(&self, f: impl Fn(&Waker) + Send + Sync + 'static) -> Waker;
}

impl WakerExt for Waker {
    fn chain(&self, f: impl Fn(&Waker) + Send + Sync + 'static) -> Waker {
        let inner = self.clone();
        waker_fn(move || (f)(&inner))
    }
}
