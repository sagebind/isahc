//! Very tiny module containing helpers for `AsyncRead` and `AsyncWrite`.

use futures_io::{AsyncRead, AsyncWrite};
use futures_lite::{
    future::{poll_fn, yield_now},
    pin,
};
use std::{
    io::{ErrorKind, Result},
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) async fn read_async<R>(reader: &mut R, buf: &mut [u8]) -> Result<usize>
where
    R: AsyncRead + Unpin,
{
    pin!(reader);

    poll_fn(|cx| reader.as_mut().poll_read(cx, buf)).await
}

pub(crate) async fn write_async<R>(writer: &mut R, buf: &[u8]) -> Result<usize>
where
    R: AsyncWrite + Unpin,
{
    pin!(writer);

    poll_fn(|cx| writer.as_mut().poll_write(cx, buf)).await
}

pub(crate) async fn write_all_async<R>(writer: &mut R, buf: &[u8]) -> Result<usize>
where
    R: AsyncWrite + Unpin,
{
    let mut amt = 0;

    while amt < buf.len() {
        match write_async(writer, &buf[amt..]).await {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(len) => amt += len,
            Err(e) if e.kind() == ErrorKind::Interrupted => yield_now().await,
            Err(e) => return Err(e),
        }
    }

    Ok(amt)
}

pub(crate) async fn copy_async<R, W>(mut reader: R, mut writer: W) -> Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = [0; 8192];

    let mut amt = 0;

    loop {
        match read_async(&mut reader, &mut buf).await {
            Ok(0) => break,
            Ok(len) => {
                amt += write_all_async(&mut writer, &buf[..len]).await? as u64;
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => yield_now().await,
            Err(e) => return Err(e),
        }
    }

    Ok(amt)
}

pub(crate) struct Sink;

impl AsyncWrite for Sink {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}
