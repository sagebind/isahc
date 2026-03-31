use event_listener::{Event, Listener};
use http::HeaderMap;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

/// Holds the current state of a trailer for a response.
///
/// This object acts as a shared handle that can be cloned and polled from
/// multiple threads to wait for and act on the response trailer.
///
/// There are two typical workflows for accessing trailer headers:
///
/// - If you are consuming the response body and then accessing the headers
///   afterward, then all trailers are guaranteed to have arrived (if any).
///   [`Trailer::try_get`] will allow you to access them without extra overhead.
/// - If you are handling trailers in a separate task, callback, or thread, then
///   either [`Trailer::wait`] or [`Trailer::wait_async`] will allow you to wait
///   for the trailer headers to arrive and then handle them.
///
/// Note that in either approach, trailer headers are delivered to your
/// application as a single [`HeaderMap`]; it is not possible to handle
/// individual headers as they arrive.
#[derive(Clone, Debug)]
pub struct Trailer {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    headers: OnceLock<HeaderMap>,
    ready: Event,
}

impl Trailer {
    /// Get a populated trailer handle containing no headers.
    pub(crate) fn empty() -> &'static Self {
        static EMPTY: OnceLock<Trailer> = OnceLock::new();

        EMPTY.get_or_init(|| Self {
            shared: Arc::new(Shared {
                headers: OnceLock::from(HeaderMap::new()),
                ready: Event::new(),
            }),
        })
    }

    /// Returns true if the trailer has been received (if any).
    ///
    /// The trailer will not be received until the body stream associated with
    /// this response has been fully consumed.
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
    ///
    /// This is a blocking operation! If you are writing an asynchronous
    /// application, then you probably want to use [`Trailer::wait_async`]
    /// instead.
    pub fn wait(&self) -> &HeaderMap {
        loop {
            // Fast path: If the headers are already set, return them.
            if let Some(headers) = self.try_get() {
                return headers;
            }

            // Headers not set, jump into the slow path by creating a new
            // listener for the ready event.
            let listener = self.shared.ready.listen();

            // Double-check that the headers are not set.
            if let Some(headers) = self.try_get() {
                return headers;
            }

            // Otherwise, block until they are set.
            listener.wait();

            // If we got the notification, then the headers are likely to be
            // set.
            if let Some(headers) = self.try_get() {
                return headers;
            }
        }
    }

    /// Block the current thread until the trailer headers arrive or a timeout
    /// expires.
    ///
    /// If the given timeout expired before the trailer arrived then `None` is
    /// returned.
    ///
    /// This is a blocking operation! If you are writing an asynchronous
    /// application, then you probably want to use [`Trailer::wait_async`]
    /// instead.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<&HeaderMap> {
        // Fast path: If the headers are already set, return them.
        if let Some(headers) = self.try_get() {
            return Some(headers);
        }

        // Headers not set, jump into the slow path by creating a new listener
        // for the ready event.
        let listener = self.shared.ready.listen();

        // Double-check that the headers are not set.
        if let Some(headers) = self.try_get() {
            return Some(headers);
        }

        // Otherwise, block with a timeout.
        if listener.wait_timeout(timeout).is_some() {
            self.try_get()
        } else {
            None
        }
    }

    /// Wait asynchronously until the trailer headers arrive, and then return
    /// them.
    pub async fn wait_async(&self) -> &HeaderMap {
        loop {
            // Fast path: If the headers are already set, return them.
            if let Some(headers) = self.try_get() {
                return headers;
            }

            // Headers not set, jump into the slow path by creating a new
            // listener for the ready event.
            let listener = self.shared.ready.listen();

            // Double-check that the headers are not set.
            if let Some(headers) = self.try_get() {
                return headers;
            }

            // Otherwise, wait asynchronously until they are.
            listener.await;

            // If we got the notification, then the headers are likely to be
            // set.
            if let Some(headers) = self.try_get() {
                return headers;
            }
        }
    }
}

pub(crate) struct TrailerWriter {
    shared: Arc<Shared>,
    headers: Option<HeaderMap>,
}

impl TrailerWriter {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                headers: Default::default(),
                ready: Event::new(),
            }),
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

    #[inline]
    pub(crate) fn flush(&mut self) {
        if !self.flush_impl() {
            tracing::warn!("tried to flush trailer multiple times");
        }
    }

    fn flush_impl(&mut self) -> bool {
        if let Some(headers) = self.headers.take() {
            let _ = self.shared.headers.set(headers);

            // Wake up any calls waiting for the headers.
            self.shared.ready.notify(usize::max_value());

            true
        } else {
            false
        }
    }
}

impl Drop for TrailerWriter {
    fn drop(&mut self) {
        self.flush_impl();
    }
}
