//! Tiny module containing utilities for working with futures.

use crossbeam_utils::sync::Parker;
use futures_lite::pin;
use std::{
    cell::RefCell,
    future::Future,
    task::{Context, Poll, Waker},
};
use waker_fn::waker_fn;

pub(crate) trait FutureExt: Future {
    /// Block the current thread until this future completes.
    fn wait(self) -> <Self as Future>::Output
    where
        Self: Sized,
    {
        fn create_parker() -> (Parker, Waker) {
            let parker = Parker::new();
            let unparker = parker.unparker().clone();
            let waker = waker_fn(move || unparker.unpark());

            (parker, waker)
        }

        thread_local! {
            static PARKER: RefCell<(Parker, Waker)> = RefCell::new(create_parker());
        }

        let future = self;
        pin!(future);

        PARKER.with(|cell| {
            if let Ok(borrow) = cell.try_borrow_mut() {
                let mut cx = Context::from_waker(&borrow.1);

                loop {
                    match future.as_mut().poll(&mut cx) {
                        Poll::Ready(result) => break result,
                        Poll::Pending => borrow.0.park(),
                    }
                }
            } else {
                let (parker, waker) = create_parker();
                let mut cx = Context::from_waker(&waker);

                loop {
                    match future.as_mut().poll(&mut cx) {
                        Poll::Ready(result) => break result,
                        Poll::Pending => parker.park(),
                    }
                }
            }
        })
    }
}

impl<F: Future> FutureExt for F {}
