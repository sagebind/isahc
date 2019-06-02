//! Task waker implementations.

use crate::Error;
use futures::task::*;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

/// Create a waker from a closure.
fn waker_fn(f: impl Fn() + 'static) -> Waker {
    struct Impl<F>(F);

    impl<F: Fn() + 'static> ArcWake for Impl<F> {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            (&arc_self.0)()
        }
    }

    Arc::new(Impl(f)).into_waker()
}

/// Helper methods for working with wakers.
pub trait WakerExt {

    /// Create a new waker from a closure that accepts this waker as an
    /// argument.
    fn chain(&self, f: impl Fn(&Waker) + 'static) -> Waker;
}

impl WakerExt for Waker {
    fn chain(&self, f: impl Fn(&Waker) + 'static) -> Waker {
        let inner = self.clone();
        waker_fn(move || {
            (f)(&inner)
        })
    }
}

/// A waker that knows how to wake up an agent thread.
pub struct AgentWaker {
    /// It's not actually magic! We actually just use UDP to send wakeup signals
    /// to an agent. Each agent listens on a unique loopback address, which is
    /// chosen randomly when the agent is created.
    socket: UdpSocket,
}

impl AgentWaker {
    /// Create a waker by connecting to the wake address of an agent.
    pub fn connect(addr: SocketAddr) -> Result<Self, Error> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.set_nonblocking(true)?;
        socket.connect(addr)?;

        Ok(Self {
            socket,
        })
    }
}

impl ArcWake for AgentWaker {
    /// Request the connected agent event loop to wake up. Just like a morning
    /// person would do.
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // We don't actually care here if this succeeds. Maybe the agent is
        // busy, or tired, or just needs some alone time right now.
        arc_self.socket.send(&[1]).is_ok();
    }
}
