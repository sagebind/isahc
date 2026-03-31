//! Shared thread pool for executing request handlers.
//!
//! While mocks can't share TCP servers since the approach of this library is
//! port-per-test, we _can_ share threads across all mock servers to make it not
//! quite as inefficient.

use std::sync::LazyLock;
use threadfin::ThreadPool;

/// Get access to the shared thread pool.
pub(crate) fn pool() -> &'static ThreadPool {
    // Pool that crates pretty much as many threads as needed, while still
    // allowing reuse.
    static POOL: LazyLock<ThreadPool> = LazyLock::new(|| ThreadPool::builder().size(..100).build());

    &POOL
}
