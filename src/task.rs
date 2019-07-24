//! Helpers for working with tasks and futures.

use crate::Error;
use futures_util::{pin_mut, task::ArcWake};
use std::future::Future;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::thread::{self, Thread};

/// Extension trait for efficiently blocking on a future.
pub(crate) trait Join: Future {
    fn join(self) -> <Self as Future>::Output;
}

impl<F: Future> Join for F {
    fn join(self) -> <Self as Future>::Output {
        struct ThreadWaker(Thread);

        impl ArcWake for ThreadWaker {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.0.unpark();
            }
        }

        let future = self;
        pin_mut!(future);
        let waker = Arc::new(ThreadWaker(thread::current())).into_waker();
        let mut context = Context::from_waker(&waker);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => thread::park(),
            }
        }
    }
}

/// Create a waker from a closure.
fn waker_fn(f: impl Fn() + Send + Sync + 'static) -> Waker {
    struct Impl<F>(F);

    impl<F: Fn() + Send + Sync + 'static> ArcWake for Impl<F> {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            (&arc_self.0)()
        }
    }

    Arc::new(Impl(f)).into_waker()
}

/// Helper methods for working with wakers.
pub(crate) trait WakerExt {
    /// Create a new waker from a closure that accepts this waker as an
    /// argument.
    fn chain(&self, f: impl Fn(&Waker) + Send + Sync + 'static) -> Waker;
}

impl WakerExt for Waker {
    fn chain(&self, f: impl Fn(&Waker) + Send + Sync + 'static) -> Waker {
        let inner = self.clone();
        waker_fn(move || (f)(&inner))
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

impl ArcWake for UdpWaker {
    /// Request the connected agent event loop to wake up. Just like a morning
    /// person would do.
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // We don't actually care here if this succeeds. Maybe the agent is
        // busy, or tired, or just needs some alone time right now.
        if let Err(e) = arc_self.socket.send(&[1]) {
            log::debug!("agent waker produced an error: {}", e);
        }
    }
}
