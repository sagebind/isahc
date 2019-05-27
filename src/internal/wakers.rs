//! Task waker implementations.

use crate::Error;
use futures::task::*;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

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
    fn wake_by_ref(arc_self: &Arc<AgentWaker>) {
        // We don't actually care here if this succeeds. Maybe the agent is
        // busy, or tired, or just needs some alone time right now.
        arc_self.socket.send(&[1]).is_ok();
    }
}

/// Waker that unpauses reads for an active request.
pub struct ReadWaker {
    token: usize,
}

/// Waker that unpauses writes for an active request.
pub struct WriteWaker {
    token: usize,
}
