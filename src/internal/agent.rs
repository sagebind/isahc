//! Curl agent that executes multiple requests simultaneously.

use crate::error::Error;
use crate::internal::notify;
use crate::internal::request::*;
use log::*;
use slab::Slab;
use std::sync::{Arc, Weak};
use std::sync::atomic::*;
use std::thread;
use std::time::{Duration, Instant};

const AGENT_THREAD_NAME: &'static str = "curl agent";
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_TIMEOUT: Duration = Duration::from_millis(1000);

/// Create an agent that executes multiple curl requests simultaneously.
///
/// The agent maintains a background thread that multiplexes all active requests using a single "multi" handle.
pub fn create() -> Result<Handle, Error> {
    let create_start = Instant::now();

    let (message_tx, message_rx) = crossbeam::channel::unbounded();
    let (notify_tx, notify_rx) = notify::create()?;
    let close_wait_group = WaitGroup::new();

    let handle_inner = Arc::new(HandleInner {
        message_tx,
        notify_tx,
        close_wait_group: Some(close_wait_group.clone()),
        thread_terminated: AtomicBool::default(),
    });
    let handle_weak = Arc::downgrade(&handle_inner);

    thread::Builder::new().name(String::from(AGENT_THREAD_NAME)).spawn(move || {
        let agent = Agent {
            multi: curl::multi::Multi::new(),
            multi_messages: crossbeam::channel::unbounded(),
            message_rx,
            notify_rx,
            requests: Slab::new(),
            close_requested: false,
            handle: handle_weak,
            _close_wait_group: close_wait_group,
        };

        debug!("agent took {:?} to start up", create_start.elapsed());

        // Intentionally panic the thread if an error occurs.
        agent.run().unwrap();
    })?;

    Ok(Handle {
        inner: handle_inner,
    })
}

/// Handle to an agent. Handles can be sent between threads, shared, and cloned.
#[derive(Clone, Debug)]
pub struct Handle {
    inner: Arc<HandleInner>,
}

/// Actual handle to an agent. Only one of these exists per agent.
#[derive(Debug)]
struct HandleInner {
    /// Used to send messages to the agent.
    message_tx: Sender<Message>,

    /// Used to wake up the agent thread while it is polling.
    notify_tx: notify::NotifySender,

    /// Indicates that the agent thread has exited.
    thread_terminated: AtomicBool,

    /// Used to await until the agent thread terminates on drop.
    close_wait_group: Option<WaitGroup>,
}

impl Handle {
    /// Begin executing a request with this agent.
    pub fn submit_request(&self, request: CurlRequest) -> Result<(), Error> {
        request.0.get_ref().set_agent(self.clone());

        self.inner.send_message(Message::BeginRequest(request))
    }

    /// Cancel a request by its token.
    pub fn cancel_request(&self, token: usize) -> Result<(), Error> {
        self.inner.send_message(Message::Cancel(token))
    }

    /// Unpause a request by its token.
    pub fn unpause_write(&self, token: usize) -> Result<(), Error> {
        self.inner.send_message(Message::UnpauseWrite(token))
    }
}

impl HandleInner {
    /// Send a message to the associated agent.
    ///
    /// If the agent is not connected, an error is returned.
    fn send_message(&self, message: Message) -> Result<(), Error> {
        if self.thread_terminated.load(Ordering::SeqCst) {
            error!("agent thread terminated prematurely");
            return Err(Error::Internal);
        }

        self.message_tx.send(message).map_err(|_| Error::Internal)?;
        self.notify_tx.notify();

        Ok(())
    }
}

impl Drop for HandleInner {
    fn drop(&mut self) {
        if self.send_message(Message::Close).is_err() {
            warn!("agent thread was already terminated");
        }

        if let Some(close_wait_group) = self.close_wait_group.take() {
            close_wait_group.wait();
        }
    }
}

/// A message sent from the main thread to the agent thread.
#[derive(Debug)]
enum Message {
    Cancel(usize),
    Close,
    BeginRequest(CurlRequest),
    UnpauseWrite(usize),
}

type MultiMessage = (usize, Result<(), curl::Error>);

/// Internal state of the agent thread.
struct Agent {
    /// A curl multi handle, of course.
    multi: curl::multi::Multi,

    /// Queue of messages from the multi handle.
    multi_messages: (Sender<MultiMessage>, Receiver<MultiMessage>),

    /// Incoming message from the main thread.
    message_rx: Receiver<Message>,

    /// Used to wake up the agent when polling.
    notify_rx: notify::NotifyReceiver,

    /// Contains all of the active requests.
    requests: Slab<curl::multi::Easy2Handle<CurlHandler>>,

    /// Indicates if the thread has been requested to stop.
    close_requested: bool,

    /// Weak reference to a handle, used to communicate back to handles.
    handle: Weak<HandleInner>,

    /// Used to await until the agent thread terminates on drop.
    _close_wait_group: WaitGroup,
}

impl Agent {
    /// Run the agent in the current thread until requested to stop.
    fn run(mut self) -> Result<(), Error> {
        let mut wait_fds = [self.notify_rx.as_wait_fd()];
        wait_fds[0].poll_on_read(true);

        debug!("agent ready");

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
                debug!("HACK: curl returned CONNECTTIMEOUT of {:?}, truncating to 1ms!", timeout);
                timeout = Duration::from_millis(1);
            }

            // Truncate the timeout to the max value.
            timeout = timeout.min(MAX_TIMEOUT);

            // Block until activity is detected or the timeout passes.
            if timeout > Duration::from_secs(0) {
                trace!("polling with timeout of {:?}", timeout);
                self.multi.wait(&mut wait_fds, timeout)?;
            }

            // We might have woken up early from the notify fd, so drain its queue.
            if self.notify_rx.drain() {
                trace!("woke up from notify fd");
            }
        }

        debug!("agent shutting down");

        self.requests.clear();
        self.multi.close()?;

        Ok(())
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
                        warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    },
                }
            } else {
                match self.message_rx.try_recv() {
                    Ok(message) => self.handle_message(message)?,
                    Err(crossbeam::channel::TryRecvError::Empty) => break,
                    Err(crossbeam::channel::TryRecvError::Disconnected) => {
                        warn!("agent handle disconnected without close message");
                        self.close_requested = true;
                        break;
                    },
                }
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: Message) -> Result<(), Error> {
        trace!("received message from agent handle: {:?}", message);

        match message {
            Message::Close => {
                trace!("agent close requested");
                self.close_requested = true;
            },
            Message::BeginRequest(request) => {
                let mut handle = self.multi.add2(request.0)?;
                let entry = self.requests.vacant_entry();

                handle.get_ref().set_token(entry.key());
                handle.set_token(entry.key())?;

                entry.insert(handle);
            },
            Message::Cancel(token) => {
                if self.requests.contains(token) {
                    let request = self.requests.remove(token);
                    let request = self.multi.remove2(request)?;
                    drop(request);
                }
            },
            Message::UnpauseWrite(token) => {
                if let Some(request) = self.requests.get(token) {
                    request.unpause_write()?;
                } else {
                    warn!("received unpause request for unknown request token: {}", token);
                }
            },
        }

        Ok(())
    }

    fn dispatch(&mut self) -> Result<(), Error> {
        self.multi.perform()?;

        self.multi.messages(|message| {
            if let Some(result) = message.result() {
                if let Ok(token) = message.token() {
                    if self.multi_messages.0.send((token, result)).is_err() {
                        error!("Multi message queue broken!");
                    }
                }
            }
        });

        for (token, result) in self.multi_messages.1.clone().try_iter() {
            match result {
                Ok(()) => self.complete_request(token)?,
                Err(e) => {
                    debug!("curl error: {}", e);
                    self.fail_request(token, e.into())?;
                },
            };
        }

        Ok(())
    }

    fn complete_request(&mut self, token: usize) -> Result<(), Error> {
        debug!("request with token {} completed", token);
        let handle = self.requests.remove(token);
        let mut handle = self.multi.remove2(handle)?;
        handle.get_mut().complete();

        Ok(())
    }

    fn fail_request(&mut self, token: usize, error: curl::Error) -> Result<(), Error> {
        let handle = self.requests.remove(token);
        let mut handle = self.multi.remove2(handle)?;
        handle.get_mut().fail(error);

        Ok(())
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.upgrade() {
            handle.thread_terminated.store(true, Ordering::SeqCst);
        }
    }
}
