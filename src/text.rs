//! Text decoding routines.

use futures_io::AsyncRead;
use futures_util::{
    future::{FutureExt, LocalBoxFuture},
    io::{AsyncReadExt},
};
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

/// A future returning a response body decoded as text.
#[allow(missing_debug_implementations)]
pub struct Text<'a>(LocalBoxFuture<'a, io::Result<String>>);

impl<'a> Text<'a> {
    pub(crate) fn new<R>(reader: &'a mut R) -> Self
    where
        R: AsyncRead + Unpin,
    {
        Text(Box::pin(async move {
            let mut buffer = String::new();
            reader.read_to_string(&mut buffer).await?;
            Ok(buffer)
        }))
    }
}

impl<'a> Future for Text<'a> {
    type Output = io::Result<String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.as_mut().0.poll_unpin(cx)
    }
}
