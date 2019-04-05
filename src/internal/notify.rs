//! File descriptors that can be used to wake up an I/O selector.

use curl::multi::WaitFd;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::*;

#[cfg(unix)]
type Stream = std::fs::File;

#[cfg(windows)]
type Stream = std::net::TcpStream;

pub fn create() -> io::Result<(NotifySender, NotifyReceiver)> {
    let (rx, tx) = {
        #[cfg(unix)] {
            use nix::{fcntl::OFlag, unistd};
            use std::fs::File;
            use std::os::unix::prelude::*;

            let (rx, tx) = unistd::pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK).map_err(|e| match e {
                nix::Error::Sys(errno) => io::Error::from(errno),
                _ => io::ErrorKind::Other.into(),
            })?;

            unsafe {
                (File::from_raw_fd(rx), File::from_raw_fd(tx))
            }
        }

        #[cfg(windows)] {
            use std::net::*;

            let listener = TcpListener::bind("127.0.0.1:0")?;
            let tx = TcpStream::connect(&listener.local_addr()?)?;
            let rx = listener.accept()?.0;
            drop(listener);

            tx.set_nonblocking(true)?;
            rx.set_nonblocking(true)?;

            (rx, tx)
        }
    };

    let notified = Arc::<AtomicBool>::default();

    Ok((
        NotifySender {
            stream: tx,
            notified: notified.clone(),
        },
        NotifyReceiver {
            stream: rx,
            notified: notified,
        },
    ))
}

#[derive(Debug)]
pub struct NotifySender {
    stream: Stream,
    notified: Arc<AtomicBool>,
}

impl NotifySender {
    pub fn notify(&self) {
        if !self.notified.swap(true, Ordering::SeqCst) {
            drop((&self.stream).write(&[1]));
        }
    }
}

#[derive(Debug)]
pub struct NotifyReceiver {
    stream: Stream,
    notified: Arc<AtomicBool>,
}

impl NotifyReceiver {
    pub fn drain(&self) -> bool {
        if !self.notified.swap(false, Ordering::SeqCst) {
            return false;
        }

        loop {
            if (&self.stream).read(&mut [0; 32]).is_err() {
                break;
            }
        }

        true
    }

    #[cfg(unix)]
    pub fn as_wait_fd(&self) -> WaitFd {
        use std::os::unix::io::AsRawFd;

        let mut fd = WaitFd::new();
        fd.set_fd(self.stream.as_raw_fd());

        fd
    }

    #[cfg(windows)]
    pub fn as_wait_fd(&self) -> WaitFd {
        use std::os::windows::io::AsRawSocket;

        let mut fd = WaitFd::new();
        fd.set_fd(self.stream.as_raw_socket());

        fd
    }
}
