//! Curl agent that executes multiple requests simultaneously.
//!
//! The agent is implemented as a single background thread attached to a
//! "handle". The handle communicates with the agent thread by using message
//! passing.
//!
//! Since request executions are driven through futures, the agent also acts as
//! a specialized task executor for tasks related to requests.

use crate::handler::RequestHandler;
use crate::task::{UdpWaker, WakerExt};
use crate::Error;
use crossbeam_channel::{Receiver, Sender};
use curl::multi::WaitFd;
use futures_util::task::ArcWake;
use slab::Slab;
use std::net::UdpSocket;
use std::sync::Arc;
use std::task::Waker;
use std::thread;
use std::time::{Duration, Instant};

const AGENT_THREAD_NAME: &str = "curl agent";
const WAIT_TIMEOUT: Duration = Duration::from_millis(100);

type EasyHandle = curl::easy::Easy2<RequestHandler>;
type MultiMessage = (usize, Result<(), curl::Error>);

/// A handle to an active agent running in a background thread.
#[derive(Debug)]
pub(crate) struct Handle {
    /// Used to send messages to the agent thread.
    message_tx: Sender<Message>,

    /// A waker that can wake up the agent thread while it is polling.
    waker: Waker,

    /// A join handle for the agent thread.
    join_handle: Option<thread::JoinHandle<Result<(), Error>>>,
}

/// Internal state of an agent thread.
///
/// The agent thread runs the primary client event loop, which is essentially a
/// traditional curl multi event loop with some extra bookkeeping and async
/// features like wakers.
struct AgentThread {
    /// A curl multi handle, of course.
    multi: curl::multi::Multi,

    /// Queue of messages from the multi handle.
    multi_messages: (Sender<MultiMessage>, Receiver<MultiMessage>),

    /// Used to send messages to the agent thread.
    message_tx: Sender<Message>,

    /// Incoming messages from the agent handle.
    message_rx: Receiver<Message>,

    /// Used to wake up the agent when polling.
    wake_socket: UdpSocket,

    /// Contains all of the active requests.
    requests: Slab<curl::multi::Easy2Handle<RequestHandler>>,

    /// Indicates if the thread has been requested to stop.
    close_requested: bool,

    /// A waker that can wake up the agent thread while it is polling.
    waker: Waker,
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

/// Create an agent that executes multiple curl requests simultaneously.
///
/// The agent maintains a background thread that multiplexes all active requests
/// using a single "multi" handle.
pub(crate) fn new() -> Result<Handle, Error> {
    let create_start = Instant::now();

    // Create an UDP socket for the agent thread to listen for wakeups on.
    let wake_socket = UdpSocket::bind("127.0.0.1:0")?;
    wake_socket.set_nonblocking(true)?;
    let wake_addr = wake_socket.local_addr()?;
    let waker = Arc::new(UdpWaker::connect(wake_addr)?).into_waker();
    log::debug!("agent waker listening on {}", wake_addr);

    let (message_tx, message_rx) = crossbeam_channel::unbounded();

    Ok(Handle {
        message_tx: message_tx.clone(),
        waker: waker.clone(),
        join_handle: Some(
            thread::Builder::new()
                .name(String::from(AGENT_THREAD_NAME))
                .spawn(move || {
                    let agent = AgentThread {
                        multi: curl::multi::Multi::new(),
                        multi_messages: crossbeam_channel::unbounded(),
                        message_tx,
                        message_rx,
                        wake_socket,
                        requests: Slab::new(),
                        close_requested: false,
                        waker,
                    };

                    log::debug!("agent took {:?} to start up", create_start.elapsed());

                    agent.run()
                })?,
        ),
    })
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
        match self.message_tx.send(message) {
            Ok(()) => {
                // Wake the agent thread up so it will check its messages soon.
                self.waker.wake_by_ref();
                Ok(())
            }
            Err(crossbeam_channel::SendError(_)) => {
                panic!("agent thread terminated prematurely");
            }
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // Request the agent thread to shut down.
        if self.send_message(Message::Close).is_err() {
            log::error!("agent thread terminated prematurely");
        }

        // Wait for the agent thread to shut down before continuing.
        if let Some(join_handle) = self.join_handle.take() {
            match join_handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => log::error!("agent thread terminated with error: {}", e),
                Err(_) => log::error!("agent thread panicked"),
            }
        }
    }
}

impl AgentThread {
    fn begin_request(&mut self, mut request: EasyHandle) -> Result<(), Error> {
        // Prepare an entry for storing this request while it executes.
        let entry = self.requests.vacant_entry();
        let id = entry.key();

        // Initialize the handler.
        request.get_mut().init(
            id,
            {
                let tx = self.message_tx.clone();

                self.waker
                    .chain(move |inner| match tx.send(Message::UnpauseRead(id)) {
                        Ok(()) => inner.wake_by_ref(),
                        Err(_) => log::warn!(
                            "agent went away while resuming read for request [id={}]",
                            id
                        ),
                    })
            },
            {
                let tx = self.message_tx.clone();

                self.waker
                    .chain(move |inner| match tx.send(Message::UnpauseWrite(id)) {
                        Ok(()) => inner.wake_by_ref(),
                        Err(_) => log::warn!(
                            "agent went away while resuming write for request [id={}]",
                            id
                        ),
                    })
            },
        );

        // Register the request with curl.
        let mut handle = self.multi.add2(request)?;
        handle.set_token(id)?;

        // Add the handle to our bookkeeping structure.
        entry.insert(handle);

        Ok(())
    }

    fn get_wait_fds(&self) -> [WaitFd; 1] {
        let mut fd = WaitFd::new();

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            fd.set_fd(self.wake_socket.as_raw_fd());
        }

        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;
            fd.set_fd(self.wake_socket.as_raw_socket());
        }

        fd.poll_on_read(true);

        [fd]
    }

    /// Polls the message channel for new messages from any agent handles.
    ///
    /// If there are no active requests right now, this function will block
    /// until a message is received.
    fn poll_messages(&mut self) -> Result<(), Error> {
        loop {
            if !self.close_requested && self.requests.is_empty() {
                match self.message_rx.recv() {
                    Ok(message) => self.handle_message(message)?,
                    _ => {
                        log::warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    }
                }
            } else {
                match self.message_rx.try_recv() {
                    Ok(message) => self.handle_message(message)?,
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        log::warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: Message) -> Result<(), Error> {
        log::trace!("received message from agent handle: {:?}", message);

        match message {
            Message::Close => self.close_requested = true,
            Message::Execute(request) => self.begin_request(request)?,
            Message::UnpauseRead(token) => {
                if let Some(request) = self.requests.get(token) {
                    request.unpause_read()?;
                } else {
                    log::warn!(
                        "received unpause request for unknown request token: {}",
                        token
                    );
                }
            }
            Message::UnpauseWrite(token) => {
                if let Some(request) = self.requests.get(token) {
                    request.unpause_write()?;
                } else {
                    log::warn!(
                        "received unpause request for unknown request token: {}",
                        token
                    );
                }
            }
        }

        Ok(())
    }

    fn dispatch(&mut self) -> Result<(), Error> {
        self.multi.perform()?;

        // Collect messages from curl about requests that have completed,
        // whether successfully or with an error.
        self.multi.messages(|message| {
            if let Some(result) = message.result() {
                if let Ok(token) = message.token() {
                    self.multi_messages.0.send((token, result)).unwrap();
                }
            }
        });

        loop {
            match self.multi_messages.1.try_recv() {
                // A request completed.
                Ok((token, result)) => {
                    let handle = self.requests.remove(token);
                    let mut handle = self.multi.remove2(handle)?;
                    handle.get_mut().on_result(result);
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => panic!(),
            }
        }

        Ok(())
    }

    fn waker_drain(&self) -> bool {
        let mut woke = false;

        while let Ok(_) = self.wake_socket.recv_from(&mut [0; 32]) {
            woke = true;
        }

        woke
    }

    /// Run the agent in the current thread until requested to stop.
    fn run(mut self) -> Result<(), Error> {
        let mut wait_fds = self.get_wait_fds();

        log::debug!("agent ready");

        // Agent main loop.
        loop {
            self.poll_messages()?;

            // Perform any pending reads or writes and handle any state changes.
            self.dispatch()?;

            if self.close_requested {
                break;
            }

            // Block until activity is detected or the timeout passes.
            self.multi.wait(&mut wait_fds, WAIT_TIMEOUT)?;

            // We might have woken up early from the notify fd, so drain its
            // queue.
            if self.waker_drain() {
                log::trace!("woke up from waker");
            }
        }

        log::debug!("agent shutting down");

        self.requests.clear();
        self.multi.close()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>() {}
    fn is_sync<T: Sync>() {}

    #[test]
    fn traits() {
        is_send::<Handle>();
        is_sync::<Handle>();

        is_send::<Message>();
    }
}
