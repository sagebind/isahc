use curl::multi::Socket;
use polling::{Event, Poller};
use slab::Slab;
use std::{io::Error, sync::Arc, task::Waker, time::Duration};

/// Asynchronous I/O selector for sockets. Used by the agent to wait for network
/// activity asynchronously, as directed by curl.
///
/// Provides an abstraction layer on a bare poller that manages the bookkeeping
/// of socket registration and translating oneshot registrations into persistent
/// registrations.
pub(crate) struct Selector {
    /// This is the poller we use to poll for socket activity!
    poller: Arc<Poller>,

    /// All of the sockets that we have beeb asked to keep track of.
    sockets: Slab<Registration>,

    /// Socket events that have occurred. We re-use this vec every call for
    /// efficiency.
    events: Vec<Event>,
}

/// Information stored about each registered socket.
struct Registration {
    socket: Socket,
    readable: bool,
    writable: bool,
}

impl Selector {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            poller: Arc::new(Poller::new()?),
            sockets: Slab::new(),
            events: Vec::new(),
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

    /// Register a socket with the selector. A new key will be returned to refer
    /// to the registration later.
    #[tracing::instrument(level = "trace", skip(self))]
    pub(crate) fn add(&mut self, socket: Socket, readable: bool, writable: bool) -> Result<usize, Error> {
        let entry = self.sockets.vacant_entry();
        let key = entry.key();

        // Add the socket to our poller.
        poller_add(&self.poller, socket, key, readable, writable)?;

        entry.insert(Registration {
            socket,
            readable,
            writable,
        });

        Ok(key)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub(crate) fn update(&mut self, key: usize, readable: bool, writable: bool) -> Result<(), Error> {
        if let Some(registration) = self.sockets.get_mut(key) {
            // Update the interest we have recorded for this socket.
            registration.readable = readable;
            registration.writable = writable;

            // Update the socket interests with our poller.
            poller_modify(&self.poller, registration.socket, key, readable, writable)
        } else {
            // Curl should never give us a key that we did not first give to
            // curl!
            tracing::warn!("update request for unknown key");

            Ok(())
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub(crate) fn remove(&mut self, key: usize) {
        // Remove this socket from our bookkeeping.
        let registration = self.sockets.remove(key);

        // There's a good chance that curl has already closed this socket, at
        // which point the poller implementation may have already forgotten
        // about this socket (e.g. epoll). Therefore if we get an error back we
        // just ignore it, as it is almost certainly benign.
        if let Err(e) = self.poller.delete(registration.socket) {
            tracing::debug!(key = key, fd = registration.socket, "error removing socket from poller: {}", e);
        }
    }

    /// Block until activity is detected or a timeout passes.
    pub(crate) fn poll(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        // Since our I/O events are oneshot, make sure we re-register sockets
        // with the poller that previously were triggered last time we polled.
        for event in self.events.drain(..) {
            if let Some(registration) = self.sockets.get(event.key) {
                poller_modify(&self.poller, registration.socket, event.key, registration.readable, registration.writable)?;
            }
        }

        // Block until either an I/O event occurs on a socket, the timeout is
        // reached, or the agent handle interrupts us.
        self.poller.wait(&mut self.events, timeout)?;

        Ok(())
    }

    pub(crate) fn events(&self) -> impl Iterator<Item = (usize, bool, bool)> + '_ {
        self.events.iter().map(|event| (
            event.key,
            event.readable,
            event.writable
        ))
    }

    pub(crate) fn get_socket(&self, key: usize) -> Option<Socket> {
        self.sockets.get(key).map(|registration| registration.socket)
    }
}

fn poller_add(poller: &Poller, socket: Socket, key: usize, readable: bool, writable: bool) -> Result<(), Error> {
    // If this errors, we retry the operation as a modification instead.
    // This is because this new socket might re-use a file descriptor that
    // was previously closed, but is still registered with the poller.
    // Retrying the operation as a modification is sufficient to handle
    // this.
    //
    // This is especially common with the epoll backend.
    if let Err(e) = poller.add(socket, Event {
        key,
        readable,
        writable,
    }) {
        tracing::debug!("failed to add interest for socket key {}, retrying as a modify: {}", key, e);
        poller.modify(socket, Event {
            key,
            readable,
            writable,
        })?;
    }

    Ok(())
}

fn poller_modify(poller: &Poller, socket: Socket, key: usize, readable: bool, writable: bool) -> Result<(), Error> {
    // If this errors, we retry the operation as an add instead. This is
    // done because epoll is weird.
    if let Err(e) = poller.modify(socket, Event {
        key,
        readable,
        writable,
    }) {
        tracing::debug!("failed to modify interest for socket key {}, retrying as an add: {}", key, e);
        poller.add(socket, Event {
            key,
            readable,
            writable,
        })?;
    }

    Ok(())
}
