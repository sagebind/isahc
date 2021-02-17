use curl::multi::Socket;
use polling::{Event, Poller};
use std::{collections::HashMap, io, sync::Arc, task::Waker, time::Duration};

const EBADF: i32 = 9;

/// Asynchronous I/O selector for sockets. Used by the agent to wait for network
/// activity asynchronously, as directed by curl.
///
/// Provides an abstraction layer on a bare poller that manages the bookkeeping
/// of socket registration and translating oneshot registrations into persistent
/// registrations.
///
/// Events are level-triggered, since that is what curl wants.
pub(crate) struct Selector {
    /// This is the poller we use to poll for socket activity!
    poller: Arc<Poller>,

    /// All of the sockets that we have been asked to keep track of.
    sockets: HashMap<Socket, Registration>,

    /// Socket events that have occurred. We re-use this vec every call for
    /// efficiency.
    events: Vec<Event>,

    /// Incrementing counter used to deduplicate registration operations.
    tick: usize,
}

/// Information stored about each registered socket.
struct Registration {
    readable: bool,
    writable: bool,
    tick: usize,
}

impl Selector {
    /// Create a new socket selector.
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            poller: Arc::new(Poller::new()?),
            sockets: HashMap::new(),
            events: Vec::new(),
            tick: 0,
        })
    }

    /// Get a task waker that will interrupt this selector whenever it is
    /// waiting for activity.
    pub(crate) fn waker(&self) -> Waker {
        waker_fn::waker_fn({
            let poller_ref = self.poller.clone();

            move || {
                let _ = poller_ref.notify();
            }
        })
    }

    /// Register a socket with the selector to begin receiving readiness events
    /// for it.
    ///
    /// This method can also be used to update/modify the readiness events you
    /// are interested in for a previously registered socket.
    #[tracing::instrument(level = "trace", skip(self))]
    pub(crate) fn register(&mut self, socket: Socket, readable: bool, writable: bool) -> io::Result<()> {
        let previous = self.sockets.insert(socket, Registration {
            readable,
            writable,
            tick: self.tick,
        });

        if previous.is_some() {
            poller_modify(&self.poller, socket, readable, writable)
        } else {
            poller_add(&self.poller, socket, readable, writable)
        }
    }

    /// Remove a socket from the selector and stop receiving events for it.
    #[tracing::instrument(level = "trace", skip(self))]
    pub(crate) fn deregister(&mut self, socket: Socket) -> io::Result<()> {
        // Remove this socket from our bookkeeping. If we recognize it, also
        // remove it from the underlying poller.
        if self.sockets.remove(&socket).is_some() {
            // There's a good chance that the socket has already been closed.
            // Depending on the poller implementation, it may have already
            // forgotten about this socket (e.g. epoll). Therefore if we get an
            // error back complaining that the socket is invalid, we can safely
            // ignore it.
            if let Err(e) = self.poller.delete(socket) {
                if e.raw_os_error() != Some(EBADF) {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Block until socket activity is detected or a timeout passes.
    ///
    /// Returns `true` if one or more socket events occurred.
    #[tracing::instrument(level = "trace", skip(self))]
    pub(crate) fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        // Since our I/O events are oneshot, make sure we re-register sockets
        // with the poller that previously were triggered last time we polled.
        //
        // We don't do this immediately after polling because the caller may
        // choose to de-register a socket before the next call. That's why we
        // wait until the last minute.
        for event in self.events.drain(..) {
            let socket = event.key as Socket;
            if let Some(registration) = self.sockets.get_mut(&socket) {
                // If the socket was already re-registered this tick, then we
                // don't need to do this.
                if registration.tick != self.tick {
                    poller_modify(&self.poller, socket, registration.readable, registration.writable)?;
                    registration.tick = self.tick;
                }
            }
        }

        self.tick = self.tick.wrapping_add(1);

        // Block until either an I/O event occurs on a socket, the timeout is
        // reached, or the agent handle interrupts us.
        match self.poller.wait(&mut self.events, Some(timeout)) {
            Ok(0) => Ok(false),
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get an iterator over the socket events that occurred during the most
    /// recent call to `poll`.
    pub(crate) fn events(&self) -> impl Iterator<Item = (Socket, bool, bool)> + '_  {
        self.events.iter().map(|event| (
            event.key as Socket,
            event.readable,
            event.writable,
        ))
    }
}

fn poller_add(poller: &Poller, socket: Socket, readable: bool, writable: bool) -> io::Result<()> {
    // If this errors, we retry the operation as a modification instead. This is
    // because this new socket might re-use a file descriptor that was
    // previously closed, but is still registered with the poller. Retrying the
    // operation as a modification is sufficient to handle this.
    //
    // This is especially common with the epoll backend.
    if let Err(e) = filter_error(poller.add(socket, Event {
        key: socket as usize,
        readable,
        writable,
    })) {
        tracing::debug!("failed to add interest for socket {}, retrying as a modify: {}", socket, e);
        filter_error(poller.modify(socket, Event {
            key: socket as usize,
            readable,
            writable,
        }))?;
    }

    Ok(())
}

fn poller_modify(poller: &Poller, socket: Socket, readable: bool, writable: bool) -> io::Result<()> {
    // If this errors, we retry the operation as an add instead. This is done
    // because epoll is weird.
    if let Err(e) = filter_error(poller.modify(socket, Event {
        key: socket as usize,
        readable,
        writable,
    })) {
        tracing::debug!("failed to modify interest for socket {}, retrying as an add: {}", socket, e);
        filter_error(poller.add(socket, Event {
            key: socket as usize,
            readable,
            writable,
        }))?;
    }

    Ok(())
}

fn filter_error(result: io::Result<()>) -> io::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(e) if is_benign_error(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

fn is_benign_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(EBADF)
}
