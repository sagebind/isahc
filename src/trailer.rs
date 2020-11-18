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
    /// Get a populated trailer handle containing no headers.
    pub(crate) fn empty() -> &'static Self {
        static EMPTY: OnceCell<Trailer> = OnceCell::new();

        EMPTY.get_or_init(|| Self {
            shared: Arc::new(Shared {
                headers: OnceCell::from(HeaderMap::new()),
                ready: Event::new(),
            })
        })
    }

    /// Returns true if the trailer has been received (if any).
    ///
    /// The trailer will not be received until the body stream associated with
    /// this response has been fully consumed. You can wait for the trailer to
    /// arrive either by `.await`-ing this handle to wait asynchronously or call
    /// [`wait`] to wait synchronously.
    #[inline]
    pub fn is_ready(&self) -> bool {
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
}

pub(crate) struct TrailerWriter {
    shared: Arc<Shared>,
    headers: Option<HeaderMap>,
}

impl TrailerWriter {
    pub(crate) fn new() -> Self {
        let shared=  Arc::new(Shared {
            headers: Default::default(),
            ready: Event::new(),
        });

        Self {
            shared,
            headers: Some(HeaderMap::new()),
        }
    }

    pub(crate) fn trailer(&self) -> Trailer {
        Trailer {
            shared: self.shared.clone(),
        }
    }

    pub(crate) fn get_mut(&mut self) -> Option<&mut HeaderMap> {
        self.headers.as_mut()
    }

    pub(crate) fn flush(&mut self) {
        if let Some(headers) = self.headers.take() {
            let _ = self.shared.headers.set(headers);

            // Wake up any calls waiting for the headers.
            self.shared.ready.notify(usize::max_value());
        } else {
            tracing::warn!("tried to flush trailer multiple times");
        }
    }
}

impl Drop for TrailerWriter {
    fn drop(&mut self) {
        self.flush();
    }
}
