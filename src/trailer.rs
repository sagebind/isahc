use event_listener::Event;
use http::HeaderMap;
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// Holds the current state of a trailer for a response.
///
/// This object acts as a shared handle that can be cloned and polled from
/// multiple threads to wait for and act on the response trailer.
#[derive(Clone, Debug)]
pub struct Trailer {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    headers: OnceCell<HeaderMap>,
    ready: Event,
}

impl Trailer {
    /// Create a new unpopulated trailer handle.
    pub(crate) fn new() -> Self {
        let shared=  Arc::new(Shared {
            headers: Default::default(),
            ready: Event::new(),
        });

        Self {
            // listener: shared.ready.listen(),
            shared,
        }
    }

    /// Get a populated trailer handle containing no headers.
    pub(crate) fn empty() -> &'static Self {
        static EMPTY: OnceCell<Trailer> = OnceCell::new();

        EMPTY.get_or_init(|| {
            let trailer = Self::new();
            trailer.set(HeaderMap::new());
            trailer
        })
    }

    /// Returns true if the trailer has been received (if any).
    ///
    /// The trailer will not be received until the body stream associated with
    /// this response has been fully consumed. You can wait for the trailer to
    /// arrive either by `.await`-ing this handle to wait asynchronously or call
    /// [`wait`] to wait synchronously.
    #[inline]
    pub fn is_available(&self) -> bool {
        self.try_get().is_some()
    }

    /// Attempt to get the trailer headers without blocking. Returns `None` if
    /// the trailer has not been received yet.
    #[inline]
    pub fn try_get(&self) -> Option<&HeaderMap> {
        self.shared.headers.get()
    }

    /// Block the current thread until the trailer headers arrive, and then
    /// return them.
    pub fn wait(&self) -> &HeaderMap {
        // Fast path: If the headers are already set, return them.
        if let Some(headers) = self.try_get() {
            return headers;
        }

        // Headers not set, jump into the slow path by creating a new listener
        // for the ready event.
        let listener = self.shared.ready.listen();

        // Double-check that the headers are not set.
        if let Some(headers) = self.try_get() {
            return headers;
        }

        // Otherwise, block until they are set.
        listener.wait();

        // If we got the notification, then the headers are guaranteed to be
        // set.
        self.try_get().unwrap()
    }

    /// Wait asynchronously until the trailer headers arrive, and then return
    /// them.
    pub async fn wait_async(&self) -> &HeaderMap {
        // Fast path: If the headers are already set, return them.
        if let Some(headers) = self.try_get() {
            return headers;
        }

        // Headers not set, jump into the slow path by creating a new listener
        // for the ready event.
        let listener = self.shared.ready.listen();

        // Double-check that the headers are not set.
        if let Some(headers) = self.try_get() {
            return headers;
        }

        // Otherwise, wait asynchronously until they are.
        listener.await;

        // If we got the notification, then the headers are guaranteed to be
        // set.
        self.try_get().unwrap()
    }

    /// Set the trailing headers for this response.
    ///
    /// This function should only be called once.
    pub(crate) fn set(&self, headers: HeaderMap) {
        if self.shared.headers.set(headers).is_err() {
            tracing::warn!("tried to flush trailer multiple times");
        } else {
            // Wake up any calls waiting for the headers.
            self.shared.ready.notify(usize::max_value());
        }
    }
}
