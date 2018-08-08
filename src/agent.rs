use curl;
use error::Error;
use futures::prelude::*;
use slab::Slab;
use std::sync::Arc;
use std::sync::atomic::*;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use transfer::*;

const DEFAULT_TIMEOUT_MS: u64 = 1000;
const AGENT_THREAD_NAME: &'static str = "curl agent";

struct Wake {}

impl Wake {
    pub fn wake(&self) {}
}

pub struct CurlTransferToken(usize);

pub struct CurlAgent {
    sender: mpsc::SyncSender<Message>,
    thread: thread::JoinHandle<Result<(), Error>>,
    drop_flag: Arc<AtomicBool>,
}

enum Message {
    Transfer(CurlTransfer),
}

impl CurlAgent {
    pub fn new() -> Result<Self, Error> {
        let (tx, rx) = mpsc::sync_channel(256);
        let drop_flag = Arc::new(AtomicBool::default());
        let drop_flag_inner = drop_flag.clone();

        let thread = thread::Builder::new().name(String::from(AGENT_THREAD_NAME)).spawn(move || {
            let thread = CurlAgentThread {
                multi: curl::multi::Multi::new(),
                drop_flag: drop_flag_inner,
                transfers: Slab::new(),
                inbox: rx,
            };

            thread.run()
        })?;

        Ok(Self {
            sender: tx,
            thread: thread,
            drop_flag: drop_flag,
        })
    }

    pub fn add(&self, transfer: CurlTransfer) {
        self.sender.send(Message::Transfer(transfer)).unwrap();
    }
}

impl Drop for CurlAgent {
    fn drop(&mut self) {
        self.drop_flag.store(true, Ordering::Relaxed);
    }
}

struct CurlAgentThread {
    multi: curl::multi::Multi,
    drop_flag: Arc<AtomicBool>,
    transfers: Slab<curl::multi::Easy2Handle<TransferState>>,
    inbox: mpsc::Receiver<Message>,
}

impl CurlAgentThread {
    fn run(mut self) -> Result<(), Error> {
        trace!("agent thread started");

        while !self.drop_flag.load(Ordering::Relaxed) {
            self.poll_messages()?;
            self.dispatch()?;
        }

        trace!("agent thread shutting down");

        self.multi.close()?;

        Ok(())
    }

    fn dispatch(&mut self) -> Result<(), Error> {
        // Determine the blocking timeout value.
        let timeout = self.multi.get_timeout()?.unwrap_or(Duration::from_millis(DEFAULT_TIMEOUT_MS));

        // Block until activity is detected or the timeout passes.
        trace!("waiting with timeout of {:?}", timeout);
        self.multi.wait(&mut [], timeout)?;

        // Perform any pending reads or writes. If `perform()` returns zero, then the current transfer is complete.
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
            let handle = self.transfers.remove(token);
            let mut handle = self.multi.remove2(handle).unwrap();

            match result {
                Ok(()) => {},
                Err(e) => {
                    debug!("libcurl error: {}", e);
                    handle.get_mut().fail(e.into());
                },
            };
        }

        Ok(())
    }

    fn poll_messages(&mut self) -> Result<(), Error> {
        // Handle pending messages.
        while let Ok(message) = self.inbox.try_recv() {
            self.handle_message(message)?;
        }

        // While there are no active transfers, we can block until we receive a message.
        while self.transfers.is_empty() {
            match self.inbox.recv() {
                Ok(message) => self.handle_message(message)?,
                Err(_) => break,
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: Message) -> Result<(), Error> {
        match message {
            Message::Transfer(transfer) => {
                let mut handle = self.multi.add2(transfer.easy)?;
                let mut entry = self.transfers.vacant_entry();

                handle.set_token(entry.key());
                entry.insert(handle);
            },
        }

        Ok(())
    }
}
