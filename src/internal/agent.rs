//! Curl agent that executes multiple requests simultaneously.
//!
//! Since request executions are driven through futures, the agent also acts as
//! a specialized task executor for tasks related to requests.

use crate::error::Error;
use crate::internal::handler::CurlHandler;
use crate::internal::wakers::{AgentWaker, WakerExt};
use crossbeam_channel::{Receiver, Sender};
use curl::multi::WaitFd;
use futures::task::*;
use futures::task::ArcWake;
use slab::Slab;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::*;
use std::thread;
use std::time::{Duration, Instant};

const AGENT_THREAD_NAME: &'static str = "curl agent";
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_TIMEOUT: Duration = Duration::from_millis(1000);

type EasyHandle = curl::easy::Easy2<CurlHandler>;
type MultiMessage = (usize, Result<(), curl::Error>);

/// A handle to an active agent running in a background thread.
pub struct Agent {
    /// Used to send messages to the agent thread.
    message_tx: Sender<Message>,

    /// A waker that can wake up the agent thread while it is polling.
    waker: Waker,

    /// State that is shared between the agent handle and the agent thread.
    shared: Arc<Shared>,
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

    /// Incoming message from the main thread.
    message_rx: Receiver<Message>,

    /// Used to wake up the agent when polling.
    wake_socket: UdpSocket,

    /// Contains all of the active requests.
    requests: Slab<curl::multi::Easy2Handle<CurlHandler>>,

    /// Indicates if the thread has been requested to stop.
    close_requested: bool,

    /// A waker that can wake up the agent thread while it is polling.
    waker: Waker,

    /// State that is shared between the agent handle and the agent thread.
    shared: Arc<Shared>,
}

/// State that is shared between the agent handle and the agent thread.
struct Shared {
    /// Flag indicating whether the agent thread has closed.
    is_closed: AtomicBool,
}

/// A message sent from the main thread to the agent thread.
#[derive(Debug)]
enum Message {
    /// Requests the agent to close.
    Close,

    /// Begin executing a new request.
    Execute(EasyHandle),

    /// Requests the agent to cancel the request with the given ID.
    Cancel(usize),

    /// Request to resume reading the request body for the request with the
    /// given ID.
    UnpauseRead(usize),

    /// Request to resume writing the response body for the request with the
    /// given ID.
    UnpauseWrite(usize),
}

impl Agent {
    /// Create an agent that executes multiple curl requests simultaneously.
    ///
    /// The agent maintains a background thread that multiplexes all active
    /// requests using a single "multi" handle.
    pub fn new() -> Result<Self, Error> {
        let create_start = Instant::now();

        // Create an UDP socket for the agent thread to listen for wakeups on.
        let wake_socket = UdpSocket::bind("127.0.0.1:0")?;
        wake_socket.set_nonblocking(true)?;
        let wake_addr = wake_socket.local_addr()?;
        let waker = Arc::new(AgentWaker::connect(wake_addr)?).into_waker();
        log::debug!("agent waker listening on {}", wake_addr);

        let (message_tx, message_rx) = crossbeam_channel::unbounded();
        let shared = Arc::new(Shared {
            is_closed: AtomicBool::default(),
        });

        let agent = Agent {
            message_tx: message_tx.clone(),
            waker: waker.clone(),
            shared: shared.clone(),
        };

        thread::Builder::new().name(String::from(AGENT_THREAD_NAME)).spawn(move || {
            let agent = AgentThread {
                multi: curl::multi::Multi::new(),
                multi_messages: crossbeam_channel::unbounded(),
                message_tx,
                message_rx,
                wake_socket,
                requests: Slab::new(),
                close_requested: false,
                waker,
                shared,
            };

            log::debug!("agent took {:?} to start up", create_start.elapsed());

            // Intentionally panic the thread if an error occurs.
            agent.run().unwrap();
        })?;

        Ok(agent)
    }

    pub fn is_closed(&self) -> bool {
        self.shared.is_closed.load(Ordering::SeqCst)
    }

    /// Get a waker object for waking up this agent's event loop from another
    /// thread.
    pub fn waker(&self) -> &Waker {
        &self.waker
    }

    /// Begin executing a request with this agent.
    pub fn submit_request(&self, request: EasyHandle) -> Result<(), Error> {
        self.send_message(Message::Execute(request))
    }

    /// Cancel a request by its token.
    pub fn cancel_request(&self, token: usize) -> Result<(), Error> {
        self.send_message(Message::Cancel(token))
    }

    /// Send a message to the agent thread.
    ///
    /// If the agent is not connected, an error is returned.
    fn send_message(&self, message: Message) -> Result<(), Error> {
        if self.is_closed() {
            log::error!("agent thread terminated prematurely");
            return Err(Error::Internal);
        }

        self.message_tx.send(message).map_err(|_| Error::Internal)?;

        // Wake the agent thread up so it will check its messages soon.
        self.waker.wake_by_ref();

        Ok(())
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if self.send_message(Message::Close).is_err() {
            log::warn!("agent thread was already terminated");
        }

        if let Some(close_wait_group) = self.close_wait_group.take() {
            close_wait_group.wait();
        }
    }
}

impl AgentThread {
    fn begin_request(&mut self, mut request: curl::easy::Easy2<CurlHandler>) -> Result<(), Error> {
        // Prepare an entry for storing this request while it executes.
        let entry = self.requests.vacant_entry();
        let id = entry.key();

        // Initialize the handler.
        request.get_mut().init(
            id,
            {
                let tx = self.message_tx.clone();

                self.waker.chain(move |inner| {
                    match tx.send(Message::UnpauseRead(id)) {
                        Ok(()) => inner.wake_by_ref(),
                        Err(_) => log::warn!("agent went away while resuming read for request [id={}]", id),
                    }
                })
            },
            {
                let tx = self.message_tx.clone();

                self.waker.chain(move |inner| {
                    match tx.send(Message::UnpauseWrite(id)) {
                        Ok(()) => inner.wake_by_ref(),
                        Err(_) => log::warn!("agent went away while resuming write for request [id={}]", id),
                    }
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

        #[cfg(unix)] {
            use std::os::unix::io::AsRawFd;
            fd.set_fd(self.wake_socket.as_raw_fd());
        }

        #[cfg(windows)] {
            use std::os::windows::io::AsRawSocket;
            fd.set_fd(self.wake_socket.as_raw_socket());
        }

        fd.poll_on_read(true);

        [fd]
    }

    /// Polls the message channel for new messages from any agent handles.
    ///
    /// If there are no active requests right now, this function will block until a message is received.
    fn poll_messages(&mut self) -> Result<(), Error> {
        loop {
            if !self.close_requested && self.requests.is_empty() {
                match self.message_rx.recv() {
                    Ok(message) => self.handle_message(message)?,
                    _ => {
                        log::warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    },
                }
            } else {
                match self.message_rx.try_recv() {
                    Ok(message) => self.handle_message(message)?,
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        log::warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    },
                }
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: Message) -> Result<(), Error> {
        log::trace!("received message from agent handle: {:?}", message);

        match message {
            Message::Close => {
                log::trace!("agent close requested");
                self.close_requested = true;
            },
            Message::Execute(request) => self.begin_request(request)?,
            Message::Cancel(token) => {
                if self.requests.contains(token) {
                    let request = self.requests.remove(token);
                    let request = self.multi.remove2(request)?;
                    drop(request);
                }
            },
            Message::UnpauseRead(token) => {
                if let Some(request) = self.requests.get(token) {
                    request.unpause_read()?;
                } else {
                    log::warn!("received unpause request for unknown request token: {}", token);
                }
            },
            Message::UnpauseWrite(token) => {
                if let Some(request) = self.requests.get(token) {
                    request.unpause_write()?;
                } else {
                    log::warn!("received unpause request for unknown request token: {}", token);
                }
            },
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
                Ok((token, Ok(()))) => self.complete_request(token)?,
                Ok((token, Err(e))) => {
                    log::debug!("curl error: {}", e);
                    self.fail_request(token, e.into())?;
                },
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => panic!(),
            }
        }

        Ok(())
    }

    fn complete_request(&mut self, token: usize) -> Result<(), Error> {
        log::debug!("request with token {} completed", token);

        let handle = self.requests.remove(token);
        let mut handle = self.multi.remove2(handle)?;
        // handle.get_mut().complete();

        Ok(())
    }

    fn fail_request(&mut self, token: usize, error: curl::Error) -> Result<(), Error> {
        let handle = self.requests.remove(token);
        let mut handle = self.multi.remove2(handle)?;
        // handle.get_mut().fail(error);

        Ok(())
    }

    fn waker_drain(&self) -> bool {
        let mut woke = false;

        loop {
            match self.wake_socket.recv_from(&mut [0; 32]) {
                Ok(_) => woke = true,
                Err(e) => break,
            }
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

            // Determine the blocking timeout value. If curl returns None, then it is unsure as to what timeout value is
            // appropriate. In this case we use a default value.
            let mut timeout = self.multi.get_timeout()?.unwrap_or(DEFAULT_TIMEOUT);

            // HACK: A mysterious bug in recent versions of curl causes it to return the value of
            // `CURLOPT_CONNECTTIMEOUT_MS` a few times during the DNS resolve phase. Work around this issue by
            // truncating this known value to 1ms to avoid blocking the agent loop for a long time.
            // See https://github.com/curl/curl/issues/2996 and https://github.com/alexcrichton/curl-rust/issues/227.
            if timeout == Duration::from_secs(300) {
                log::debug!("HACK: curl returned CONNECTTIMEOUT of {:?}, truncating to 1ms!", timeout);
                timeout = Duration::from_millis(1);
            }

            // Truncate the timeout to the max value.
            timeout = timeout.min(MAX_TIMEOUT);

            // Block until activity is detected or the timeout passes.
            if timeout > Duration::from_secs(0) {
                log::trace!("polling with timeout of {:?}", timeout);
                self.multi.wait(&mut wait_fds, timeout)?;
            }

            // We might have woken up early from the notify fd, so drain its queue.
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

impl Drop for AgentThread {
    fn drop(&mut self) {
        self.shared.is_closed.store(true, Ordering::SeqCst);
    }
}
