//! Curl agent that executes multiple requests simultaneously.
//!
//! The agent is implemented as a single background thread attached to a
//! "handle". The handle communicates with the agent thread by using message
//! passing. The agent executes multiple curl requests simultaneously by using a
//! single "multi" handle.
//!
//! Since request executions are driven through futures, the agent also acts as
//! a specialized task executor for tasks related to requests.

use crate::{error::Error, handler::RequestHandler, task::WakerExt};
use async_channel::{Receiver, Sender};
use crossbeam_utils::{atomic::AtomicCell, sync::WaitGroup};
use curl::multi::{Events, Multi, Socket, SocketEvents};
use futures_lite::future::block_on;
use slab::Slab;
use std::{
    io,
    sync::{Arc, Mutex},
    task::Waker,
    thread,
    time::{Duration, Instant},
};

use self::{selector::Selector, timer::Timer};

mod selector;
mod timer;

static NEXT_AGENT_ID: AtomicCell<usize> = AtomicCell::new(0);
const WAIT_TIMEOUT: Duration = Duration::from_millis(1000);

type EasyHandle = curl::easy::Easy2<RequestHandler>;

/// Builder for configuring and spawning an agent.
#[derive(Debug, Default)]
pub(crate) struct AgentBuilder {
    max_connections: usize,
    max_connections_per_host: usize,
    connection_cache_size: usize,
}

impl AgentBuilder {
    pub(crate) fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    pub(crate) fn max_connections_per_host(mut self, max: usize) -> Self {
        self.max_connections_per_host = max;
        self
    }

    pub(crate) fn connection_cache_size(mut self, size: usize) -> Self {
        self.connection_cache_size = size;
        self
    }

    /// Spawn a new agent using the configuration in this builder and return a
    /// handle for communicating with the agent.
    pub(crate) fn spawn(&self) -> io::Result<Handle> {
        let create_start = Instant::now();

        // Initialize libcurl, if necessary, on the current thread.
        //
        // Note that as of 0.4.30, the curl crate will attempt to do this for us
        // on the main thread automatically at program start on most targets,
        // but on other targets must still be initialized on the main thread. We
        // do this here in the hope that the user builds an `HttpClient` on the
        // main thread (as opposed to waiting for `Multi::new()` to do it for
        // us below, which we _know_ is not on the main thread).
        //
        // See #189.
        curl::init();

        let id = NEXT_AGENT_ID.fetch_add(1);

        // Create an I/O selector for driving curl's sockets.
        let selector = Selector::new()?;

        let (message_tx, message_rx) = async_channel::unbounded();

        let wait_group = WaitGroup::new();
        let wait_group_thread = wait_group.clone();

        let max_connections = self.max_connections;
        let max_connections_per_host = self.max_connections_per_host;
        let connection_cache_size = self.connection_cache_size;

        // Create a span for the agent thread that outlives this method call,
        // but rather was caused by it.
        let agent_span = tracing::debug_span!("agent_thread", id);
        agent_span.follows_from(tracing::Span::current());

        let waker = selector.waker();
        let message_tx_clone = message_tx.clone();

        let thread_main = move || {
            let _enter = agent_span.enter();
            let mut multi = Multi::new();

            if max_connections > 0 {
                multi
                    .set_max_total_connections(max_connections)
                    .map_err(Error::from_any)?;
            }

            if max_connections_per_host > 0 {
                multi
                    .set_max_host_connections(max_connections_per_host)
                    .map_err(Error::from_any)?;
            }

            // Only set maxconnects if greater than 0, because 0 actually means unlimited.
            if connection_cache_size > 0 {
                multi
                    .set_max_connects(connection_cache_size)
                    .map_err(Error::from_any)?;
            }

            let agent = AgentContext::new(multi, selector, message_tx_clone, message_rx)?;

            drop(wait_group_thread);

            tracing::debug!("agent took {:?} to start up", create_start.elapsed());

            let result = agent.run();

            if let Err(e) = &result {
                tracing::error!("agent shut down with error: {:?}", e);
            }

            result
        };

        let handle = Handle {
            message_tx,
            waker,
            join_handle: Mutex::new(Some(
                thread::Builder::new()
                    .name(format!("isahc-agent-{}", id))
                    .spawn(thread_main)?,
            )),
        };

        // Block until the agent thread responds.
        wait_group.wait();

        Ok(handle)
    }
}

/// A handle to an active agent running in a background thread.
///
/// Dropping the handle will cause the agent thread to shut down and abort any
/// pending transfers.
#[derive(Debug)]
pub(crate) struct Handle {
    /// Used to send messages to the agent thread.
    message_tx: Sender<Message>,

    /// A waker that can wake up the agent thread while it is polling.
    waker: Waker,

    /// A join handle for the agent thread.
    join_handle: Mutex<Option<thread::JoinHandle<Result<(), Error>>>>,
}

/// Internal state of an agent thread.
///
/// The agent thread runs the primary client event loop, which is essentially a
/// traditional curl multi event loop with some extra bookkeeping and async
/// features like wakers.
struct AgentContext {
    /// A curl multi handle, of course.
    multi: curl::multi::Multi,

    /// Used to send messages to the agent thread.
    message_tx: Sender<Message>,

    /// Incoming messages from the agent handle.
    message_rx: Receiver<Message>,

    /// Contains all of the active requests.
    requests: Slab<curl::multi::Easy2Handle<RequestHandler>>,

    /// Indicates if the thread has been requested to stop.
    close_requested: bool,

    /// A waker that can wake up the agent thread while it is polling.
    waker: Waker,

    /// This is the poller we use to poll for socket activity!
    selector: Selector,

    /// A timer we use to keep track of curl's timeouts.
    timer: Arc<Timer>,

    /// Queue of socket registration updates from the multi handle.
    socket_updates: Receiver<(Socket, SocketEvents, usize)>,
}

/// A message sent from the main thread to the agent thread.
#[derive(Debug)]
enum Message {
    /// Requests the agent to close.
    Close,

    /// Begin executing a new request.
    Execute(EasyHandle),

    /// Request to resume reading the request body for the request with the
    /// given ID.
    UnpauseRead(usize),

    /// Request to resume writing the response body for the request with the
    /// given ID.
    UnpauseWrite(usize),
}

#[derive(Debug)]
enum JoinResult {
    AlreadyJoined,
    Ok,
    Err(Error),
    Panic,
}

impl Handle {
    /// Begin executing a request with this agent.
    pub(crate) fn submit_request(&self, request: EasyHandle) -> Result<(), Error> {
        self.send_message(Message::Execute(request))
    }

    /// Send a message to the agent thread.
    ///
    /// If the agent is not connected, an error is returned.
    fn send_message(&self, message: Message) -> Result<(), Error> {
        match self.message_tx.try_send(message) {
            Ok(()) => {
                // Wake the agent thread up so it will check its messages soon.
                self.waker.wake_by_ref();
                Ok(())
            }
            Err(_) => match self.try_join() {
                JoinResult::Err(e) => panic!("agent thread terminated with error: {:?}", e),
                JoinResult::Panic => panic!("agent thread panicked"),
                _ => panic!("agent thread terminated prematurely"),
            },
        }
    }

    fn try_join(&self) -> JoinResult {
        let mut option = self.join_handle.lock().unwrap();

        if let Some(join_handle) = option.take() {
            match join_handle.join() {
                Ok(Ok(())) => JoinResult::Ok,
                Ok(Err(e)) => JoinResult::Err(e),
                Err(_) => JoinResult::Panic,
            }
        } else {
            JoinResult::AlreadyJoined
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // Request the agent thread to shut down.
        if self.send_message(Message::Close).is_err() {
            tracing::error!("agent thread terminated prematurely");
        }

        // Wait for the agent thread to shut down before continuing.
        match self.try_join() {
            JoinResult::Ok => tracing::trace!("agent thread joined cleanly"),
            JoinResult::Err(e) => tracing::error!("agent thread terminated with error: {}", e),
            JoinResult::Panic => tracing::error!("agent thread panicked"),
            _ => {}
        }
    }
}

impl AgentContext {
    fn new(
        mut multi: Multi,
        selector: Selector,
        message_tx: Sender<Message>,
        message_rx: Receiver<Message>,
    ) -> Result<Self, Error> {
        let timer = Arc::new(Timer::new());
        let (socket_updates_tx, socket_updates_rx) = async_channel::unbounded();

        multi
            .socket_function(move |socket, events, key| {
                let _ = socket_updates_tx.try_send((socket, events, key));
            })
            .map_err(Error::from_any)?;

        multi
            .timer_function({
                let timer = timer.clone();

                move |timeout| match timeout {
                    Some(timeout) => {
                        timer.start(timeout);
                        true
                    }
                    None => {
                        timer.stop();
                        true
                    }
                }
            })
            .map_err(Error::from_any)?;

        Ok(Self {
            multi,
            message_tx,
            message_rx,
            requests: Slab::new(),
            close_requested: false,
            waker: selector.waker(),
            selector,
            timer,
            socket_updates: socket_updates_rx,
        })
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn begin_request(&mut self, mut request: EasyHandle) -> Result<(), Error> {
        // Prepare an entry for storing this request while it executes.
        let entry = self.requests.vacant_entry();
        let id = entry.key();
        let handle = request.raw();

        // Initialize the handler.
        request.get_mut().init(
            id,
            handle,
            {
                let tx = self.message_tx.clone();

                self.waker
                    .chain(move |inner| match tx.try_send(Message::UnpauseRead(id)) {
                        Ok(()) => inner.wake_by_ref(),
                        Err(_) => {
                            tracing::warn!(id, "agent went away while resuming read for request")
                        }
                    })
            },
            {
                let tx = self.message_tx.clone();

                self.waker
                    .chain(move |inner| match tx.try_send(Message::UnpauseWrite(id)) {
                        Ok(()) => inner.wake_by_ref(),
                        Err(_) => {
                            tracing::warn!(id, "agent went away while resuming write for request")
                        }
                    })
            },
        );

        // Register the request with curl.
        let mut handle = self.multi.add2(request).map_err(Error::from_any)?;
        handle.set_token(id).map_err(Error::from_any)?;

        // Add the handle to our bookkeeping structure.
        entry.insert(handle);

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn complete_request(
        &mut self,
        token: usize,
        result: Result<(), curl::Error>,
    ) -> Result<(), Error> {
        let handle = self.requests.remove(token);
        let mut handle = self.multi.remove2(handle).map_err(Error::from_any)?;

        handle.get_mut().set_result(result.map_err(Error::from_any));

        Ok(())
    }

    /// Polls the message channel for new messages from any agent handles.
    ///
    /// If there are no active requests right now, this function will block
    /// until a message is received.
    #[tracing::instrument(level = "trace", skip(self))]
    fn poll_messages(&mut self) -> Result<(), Error> {
        while !self.close_requested {
            if self.requests.is_empty() {
                match block_on(self.message_rx.recv()) {
                    Ok(message) => self.handle_message(message)?,
                    _ => {
                        tracing::warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    }
                }
            } else {
                match self.message_rx.try_recv() {
                    Ok(message) => self.handle_message(message)?,
                    Err(async_channel::TryRecvError::Empty) => break,
                    Err(async_channel::TryRecvError::Closed) => {
                        tracing::warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn handle_message(&mut self, message: Message) -> Result<(), Error> {
        tracing::trace!("received message from agent handle");

        match message {
            Message::Close => self.close_requested = true,
            Message::Execute(request) => self.begin_request(request)?,
            Message::UnpauseRead(token) => {
                if let Some(request) = self.requests.get(token) {
                    if let Err(e) = request.unpause_read() {
                        // If unpausing returned an error, it is likely because
                        // curl called our callback inline and the callback
                        // returned an error. Unfortunately this does not affect
                        // the normal state of the transfer, so we need to keep
                        // the transfer alive until it errors through the normal
                        // means, which is likely to happen this turn of the
                        // event loop anyway.
                        tracing::debug!(id = token, "error unpausing read for request: {:?}", e);
                    }
                } else {
                    tracing::warn!(
                        "received unpause request for unknown request token: {}",
                        token
                    );
                }
            }
            Message::UnpauseWrite(token) => {
                if let Some(request) = self.requests.get(token) {
                    if let Err(e) = request.unpause_write() {
                        // If unpausing returned an error, it is likely because
                        // curl called our callback inline and the callback
                        // returned an error. Unfortunately this does not affect
                        // the normal state of the transfer, so we need to keep
                        // the transfer alive until it errors through the normal
                        // means, which is likely to happen this turn of the
                        // event loop anyway.
                        tracing::debug!(id = token, "error unpausing write for request: {:?}", e);
                    }
                } else {
                    tracing::warn!(
                        "received unpause request for unknown request token: {}",
                        token
                    );
                }
            }
        }

        Ok(())
    }

    /// Run the agent in the current thread until requested to stop.
    fn run(mut self) -> Result<(), Error> {
        let mut multi_messages = Vec::new();

        // Agent main loop.
        loop {
            self.poll_messages()?;

            if self.close_requested {
                break;
            }

            // Block until activity is detected or the timeout passes.
            self.poll()?;

            // Collect messages from curl about requests that have completed,
            // whether successfully or with an error.
            self.multi.messages(|message| {
                if let Some(result) = message.result() {
                    if let Ok(token) = message.token() {
                        multi_messages.push((token, result));
                    }
                }
            });

            for (token, result) in multi_messages.drain(..) {
                self.complete_request(token, result)?;
            }
        }

        tracing::debug!("agent shutting down");

        self.requests.clear();

        Ok(())
    }

    /// Block until activity is detected or a timeout passes.
    fn poll(&mut self) -> Result<(), Error> {
        let now = Instant::now();
        let timeout = self.timer.get_remaining(now);

        // Get the latest timeout value from curl that we should use, limited to
        // a maximum we chose.
        let poll_timeout = timeout.map(|t| t.min(WAIT_TIMEOUT)).unwrap_or(WAIT_TIMEOUT);

        // Block until either an I/O event occurs on a socket, the timeout is
        // reached, or the agent handle interrupts us.
        if self.selector.poll(poll_timeout)? {
            // At least one I/O event occurred, handle them.
            for (socket, readable, writable) in self.selector.events() {
                tracing::trace!(socket, readable, writable, "socket event");
                let mut events = Events::new();
                events.input(readable);
                events.output(writable);
                self.multi
                    .action(socket, &events)
                    .map_err(Error::from_any)?;
            }
        }

        // If curl gave us a timeout, check if it has expired.
        if self.timer.is_expired(now) {
            self.timer.stop();
            self.multi.timeout().map_err(Error::from_any)?;
        }

        // Apply any requested socket updates now.
        while let Ok((socket, events, _)) = self.socket_updates.try_recv() {
            // Curl is asking us to stop polling this socket.
            if events.remove() {
                self.selector.deregister(socket).unwrap();
            } else {
                let readable = events.input() || events.input_and_output();
                let writable = events.output() || events.input_and_output();

                self.selector.register(socket, readable, writable).unwrap();
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static_assertions::assert_impl_all!(Handle: Send, Sync);
    static_assertions::assert_impl_all!(Message: Send);
}
