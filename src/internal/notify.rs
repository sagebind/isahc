//! File descriptors that can be used to wake up an I/O selector.

use std::io::{self, Read, Write};
use std::sync::atomic::*;
use std::sync::Arc;

#[cfg(windows)]
pub fn create() -> io::Result<(NotifySender, NotifyReceiver)> {
    use std::net::*;

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let sender = TcpStream::connect(&listener.local_addr()?)?;
    let receiver = listener.accept()?.0;
    drop(listener);

    sender.set_nonblocking(true)?;
    receiver.set_nonblocking(true)?;

    let notified = Arc::<AtomicBool>::default();

    Ok((
        NotifySender {
            stream: sender,
            notified: notified.clone(),
        },
        NotifyReceiver {
            stream: receiver,
            notified: notified,
        },
    ))
}

#[cfg(unix)]
pub fn create() -> io::Result<(NotifySender, NotifyReceiver)> {
    use nix;
    use std::fs::File;
    use std::os::unix::prelude::*;

    let (read_fd, write_fd) = nix::unistd::pipe2(
        nix::fcntl::OFlag::O_CLOEXEC | nix::fcntl::OFlag::O_NONBLOCK,
    ).map_err(|e| match e {
        nix::Error::Sys(errno) => io::Error::from(errno),
        _ => io::ErrorKind::Other.into(),
    })?;

    let notified = Arc::<AtomicBool>::default();

    unsafe {
        Ok((
            NotifySender {
                stream: File::from_raw_fd(write_fd),
                notified: notified.clone(),
            },
            NotifyReceiver {
                stream: File::from_raw_fd(read_fd),
                notified: notified,
            },
        ))
    }
}

pub struct NotifySender {
    #[cfg(unix)]
    stream: ::std::fs::File,

    #[cfg(windows)]
    stream: ::std::net::TcpStream,

    notified: Arc<AtomicBool>,
}

impl NotifySender {
    pub fn notify(&self) {
        if !self.notified.swap(true, Ordering::SeqCst) {
            drop((&self.stream).write(&[1]));
        }
    }
}

pub struct NotifyReceiver {
    #[cfg(unix)]
    stream: ::std::fs::File,

    #[cfg(windows)]
    stream: ::std::net::TcpStream,

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
}

#[cfg(unix)]
impl ::std::os::unix::io::AsRawFd for NotifyReceiver {
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        self.stream.as_raw_fd()
    }
}

#[cfg(windows)]
impl ::std::os::windows::io::AsRawSocket for NotifyReceiver {
    fn as_raw_handle(&self) -> ::std::os::windows::io::RawSocket {
        self.stream.as_raw_handle()
    }
}
