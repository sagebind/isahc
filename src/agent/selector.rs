use curl::multi::Socket;
use polling::{Event, Poller};
use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasherDefault, Hasher},
    io,
    sync::Arc,
    task::Waker,
    time::Duration,
};

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
    sockets: HashMap<Socket, Registration, BuildHasherDefault<IntHasher>>,

    /// If a socket is currently invalid when it is registered, we put it in this
    /// set and try to register it again later.
    bad_sockets: HashSet<Socket, BuildHasherDefault<IntHasher>>,

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
            sockets: HashMap::with_hasher(Default::default()),
            bad_sockets: HashSet::with_hasher(Default::default()),
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
    pub(crate) fn register(
        &mut self,
        socket: Socket,
        readable: bool,
        writable: bool,
    ) -> io::Result<()> {
        let previous = self.sockets.insert(socket, Registration {
            readable,
            writable,
            tick: self.tick,
        });

        let result = if previous.is_some() {
            poller_modify(&self.poller, socket, readable, writable)
        } else {
            poller_add(&self.poller, socket, readable, writable)
        };

        match result {
            Err(e) if is_bad_socket_error(&e) => {
                // We've been asked to monitor a socket, but the poller thinks
                // that the socket is invalid or closed. On occasion, curl will
                // give such sockets with the assumption that we will monitor
                // them until curl tells us to stop. With stateless pollers such
                // as `select(2)` this is not a problem, but most
                // high-performance stateless pollers need the socket to be
                // valid in order to monitor them.
                //
                // To get around this problem, we return `Ok` to the caller and
                // hold onto this currently invalid socket for later. Whenever
                // `poll` is called, we retry registering these sockets in the
                // hope that they will eventually become valid.
                tracing::debug!(socket, error = ?e, "bad socket registered, will try again later");
                self.bad_sockets.insert(socket);
                Ok(())
            }
            result => result,
        }
    }

    /// Remove a socket from the selector and stop receiving events for it.
    pub(crate) fn deregister(&mut self, socket: Socket) -> io::Result<()> {
        // Remove this socket from our bookkeeping. If we recognize it, also
        // remove it from the underlying poller.
        if self.sockets.remove(&socket).is_some() {
            self.bad_sockets.remove(&socket);

            // There's a good chance that the socket has already been closed.
            // Depending on the poller implementation, it may have already
            // forgotten about this socket (e.g. epoll). Therefore if we get an
            // error back complaining that the socket is invalid, we can safely
            // ignore it.
            if let Err(e) = self.poller.delete(socket) {
                if !is_bad_socket_error(&e) && e.kind() != io::ErrorKind::PermissionDenied {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Block until socket activity is detected or a timeout passes.
    ///
    /// Returns `true` if one or more socket events occurred.
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
                    poller_modify(
                        &self.poller,
                        socket,
                        registration.readable,
                        registration.writable,
                    )?;
                    registration.tick = self.tick;
                }
            }
        }

        // Iterate over sockets that have been registered, but failed to be
        // added to the underlying poller temporarily, and retry adding them.
        self.bad_sockets.retain({
            let sockets = &mut self.sockets;
            let poller = &self.poller;
            let tick = self.tick;

            move |&socket| {
                if let Some(registration) = sockets.get_mut(&socket) {
                    if registration.tick != tick {
                        registration.tick = tick;
                        poller_add(poller, socket, registration.readable, registration.writable)
                            .is_err()
                    } else {
                        true
                    }
                } else {
                    false
                }
            }
        });

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
    pub(crate) fn events(&self) -> impl Iterator<Item = (Socket, bool, bool)> + '_ {
        self.events
            .iter()
            .map(|event| (event.key as Socket, event.readable, event.writable))
    }
}

fn poller_add(poller: &Poller, socket: Socket, readable: bool, writable: bool) -> io::Result<()> {
    // If this errors, we retry the operation as a modification instead. This is
    // because this new socket might re-use a file descriptor that was
    // previously closed, but is still registered with the poller. Retrying the
    // operation as a modification is sufficient to handle this.
    //
    // This is especially common with the epoll backend.
    if let Err(e) = poller.add(socket, Event {
        key: socket as usize,
        readable,
        writable,
    }) {
        tracing::debug!(
            "failed to add interest for socket {}, retrying as a modify: {}",
            socket,
            e
        );
        poller.modify(socket, Event {
            key: socket as usize,
            readable,
            writable,
        })?;
    }

    Ok(())
}

fn poller_modify(
    poller: &Poller,
    socket: Socket,
    readable: bool,
    writable: bool,
) -> io::Result<()> {
    // If this errors, we retry the operation as an add instead. This is done
    // because epoll is weird.
    if let Err(e) = poller.modify(socket, Event {
        key: socket as usize,
        readable,
        writable,
    }) {
        tracing::debug!(
            "failed to modify interest for socket {}, retrying as an add: {}",
            socket,
            e
        );
        poller.add(socket, Event {
            key: socket as usize,
            readable,
            writable,
        })?;
    }

    Ok(())
}

fn is_bad_socket_error(error: &io::Error) -> bool {
    // OS-specific error codes that aren't mapped to an `std::io::ErrorKind`.
    const EBADF: i32 = 9;
    const ERROR_INVALID_HANDLE: i32 = 6;
    const ERROR_NOT_FOUND: i32 = 1168;

    match error.kind() {
        // Common error codes std understands.
        io::ErrorKind::NotFound | io::ErrorKind::InvalidInput => true,

        // Check for OS-specific error codes.
        _ => match error.raw_os_error() {
            // kqueue likes to return EBADF, especially on removal, since it
            // automatically removes sockets when they are closed.
            Some(EBADF) if cfg!(unix) => true,

            // IOCP can return these in rare circumstances. Typically these just
            // indicate that the socket is no longer registered with the
            // completion port or was already closed.
            Some(ERROR_INVALID_HANDLE) | Some(ERROR_NOT_FOUND) if cfg!(windows) => true,

            _ => false,
        },
    }
}

/// Trivial hash function to use for our maps and sets that use file descriptors
/// as keys.
#[derive(Default)]
struct IntHasher([u8; 8], #[cfg(debug_assertions)] bool);

impl Hasher for IntHasher {
    fn write(&mut self, bytes: &[u8]) {
        #[cfg(debug_assertions)]
        {
            if self.1 {
                panic!("socket hash function can only be written to once");
            } else {
                self.1 = true;
            }

            if bytes.len() > 8 {
                panic!("only a maximum of 8 bytes can be hashed");
            }
        }

        (&mut self.0[..bytes.len()]).copy_from_slice(bytes);
    }

    #[inline]
    fn finish(&self) -> u64 {
        u64::from_ne_bytes(self.0)
    }
}
