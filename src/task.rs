//! Helpers for working with tasks and futures.

use crate::Error;
use std::{
    net::{SocketAddr, UdpSocket},
    task::Waker,
};

/// Helper methods for working with wakers.
pub(crate) trait WakerExt {
    /// Create a new waker from a closure that accepts this waker as an
    /// argument.
    fn chain(&self, f: impl Fn(&Waker) + Send + Sync + 'static) -> Waker;
}

impl WakerExt for Waker {
    fn chain(&self, f: impl Fn(&Waker) + Send + Sync + 'static) -> Waker {
        let inner = self.clone();
        waker_fn::waker_fn(move || (f)(&inner))
    }
}

/// A waker that sends a signal to an UDP socket.
///
/// This kind of waker is used to wake up agent threads while they are polling.
/// Each agent listens on a unique loopback address, which is chosen randomly
/// when the agent is created.
pub(crate) struct UdpWaker {
    socket: UdpSocket,
}

impl UdpWaker {
    /// Create a waker by connecting to the wake address of an UDP server.
    pub(crate) fn connect(addr: SocketAddr) -> Result<Self, Error> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.connect(addr)?;

        Ok(Self { socket })
    }
}

impl From<UdpWaker> for Waker {
    fn from(waker: UdpWaker) -> Self {
        waker_fn::waker_fn(move || {
            // We don't actually care here if this succeeds. Maybe the agent is
            // busy, or tired, or just needs some alone time right now.
            if let Err(e) = waker.socket.send(&[1]) {
                tracing::debug!("agent waker produced an error: {}", e);
            }
        })
    }
}
