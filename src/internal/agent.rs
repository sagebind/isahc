//! Curl agent that executes multiple requests simultaneously.

use curl;
use curl::multi::WaitFd;
use error::Error;
use slab::Slab;
use std::slice;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use super::notify;
use super::request::*;

const AGENT_THREAD_NAME: &'static str = "curl agent";
const DEFAULT_TIMEOUT_MS: u64 = 1000;

/// An agent that executes multiple curl requests simultaneously.
///
/// The agent maintains a background thread that multiplexes all active requests using a single "multi" handle.
pub struct CurlAgent {
    /// Used to send messages to the agent thread.
    message_tx: mpsc::Sender<Message>,

    /// Used to wake up the agent thread while it is polling.
    notify_tx: notify::NotifySender,
}

impl CurlAgent {
    /// Create a new agent.
    pub fn new() -> Result<Self, Error> {
        let (message_tx, message_rx) = mpsc::channel();
        let (notify_tx, notify_rx) = notify::create()?;

        thread::Builder::new().name(String::from(AGENT_THREAD_NAME)).spawn(move || {
            CurlAgentThread {
                multi: curl::multi::Multi::new(),
                message_rx,
                notify_rx,
                requests: Slab::new(),
                stop: false,
            }.run()
        })?;

        Ok(Self {
            message_tx,
            notify_tx,
        })
    }

    /// Begin executing a request with this agent.
    pub fn begin_execute(&mut self, request: CurlRequest) -> Result<(), Error> {
        if self.message_tx.send(Message::Request(request)).is_err() {
            error!("agent disconnected prematurely");
            return Err(Error::Internal);
        }

        self.notify_tx.notify();

        Ok(())
    }
}

/// A message sent from the main thread to the agent thread.
enum Message {
    Request(CurlRequest),
}

/// Internal state of the agent thread.
struct CurlAgentThread {
    /// A curl multi handle, of course.
    multi: curl::multi::Multi,

    /// Incoming message from the main thread.
    message_rx: mpsc::Receiver<Message>,

    /// Used to wake up the agent when polling.
    notify_rx: notify::NotifyReceiver,

    /// Contains all of the active requests.
    requests: Slab<curl::multi::Easy2Handle<TransferState>>,

    /// Indicates if the thread has been requested to stop.
    stop: bool,
}

impl CurlAgentThread {
    /// Run the agent in the current thread until requested to stop.
    fn run(mut self) -> Result<(), Error> {
        #[allow(unused_assignments)]
        let mut wait_fd = None;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;

            let mut fd = WaitFd::new();
            fd.set_fd(self.notify_rx.as_raw_fd());
            fd.poll_on_read(true);

            wait_fd = Some(fd);
        }

        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;

            let mut fd = WaitFd::new();
            fd.set_fd(self.notify_rx.as_raw_socket() as i32);
            fd.poll_on_read(true);

            wait_fd = Some(fd);
        }

        let wait_fds = match wait_fd.as_mut() {
            Some(mut fd) => slice::from_mut(fd),
            None => {
                warn!("polling interruption is not supported on your platform");
                &mut []
            },
        };

        debug!("agent ready");

        // Agent main loop.
        while !self.stop {
            self.poll_messages()?;

            // Determine the blocking timeout value.
            let timeout = self.multi.get_timeout()?.unwrap_or(Duration::from_millis(DEFAULT_TIMEOUT_MS));

            // Block until activity is detected or the timeout passes.
            trace!("polling with timeout of {:?}", timeout);
            self.multi.wait(wait_fds, timeout)?;

            // We might have woken up early from the notify fd, so drain its queue.
            if self.notify_rx.drain() {
                trace!("woke up from notify fd");
            }

            // Perform any pending reads or writes and handle any state changes.
            self.dispatch()?;
        }

        debug!("agent shutting down");

        self.multi.close()?;

        Ok(())
    }

    fn dispatch(&mut self) -> Result<(), Error> {
        self.multi.perform()?;

        let mut messages = Vec::new();
        self.multi.messages(|message| {
            if let Some(result) = message.result() {
                if let Ok(token) = message.token() {
                    messages.push((token, result));
                }
            }
        });

        for (token, result) in messages {
            let handle = self.requests.remove(token);
            let mut handle = self.multi.remove2(handle).unwrap();

            match result {
                Ok(()) => {},
                Err(e) => {
                    debug!("curl error: {}", e);
                    handle.get_mut().fail(e.into());
                },
            };
        }

        Ok(())
    }

    fn poll_messages(&mut self) -> Result<(), Error> {
        // Handle pending messages.
        loop {
            match self.message_rx.try_recv() {
                Ok(message) => self.handle_message(message)?,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    trace!("agent handle disconnected");
                    self.stop = true;
                    break;
                },
            }
        }

        // While there are no active transfers, we can block until we receive a message.
        while self.requests.is_empty() {
            match self.message_rx.recv() {
                Ok(message) => self.handle_message(message)?,
                Err(_) => {
                    trace!("agent handle disconnected");
                    self.stop = true;
                    break;
                },
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: Message) -> Result<(), Error> {
        match message {
            Message::Request(request) => {
                let mut handle = self.multi.add2(request.0)?;
                let mut entry = self.requests.vacant_entry();

                handle.set_token(entry.key())?;
                entry.insert(handle);
            },
        }

        Ok(())
    }
}
