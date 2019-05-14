use crate::body::Body;
use crate::internal::format_byte_string;
use crate::internal::parse;
use crate::internal::response::ResponseProducer;
use bytes::Bytes;
use curl::easy::InfoType;
use futures::io::AsyncRead;
use futures::lock::{Mutex, MutexLockFuture};
use futures::task::AtomicWaker;
use std::io::{self, Read};
use std::pin::Pin;
use std::sync::Arc;
use std::task::*;

struct State {
    buffer: Mutex<Bytes>,
    read_waker: AtomicWaker,
}

pub struct CurlHandler {
    request_body: Body,
    producer: ResponseProducer,
}

impl CurlHandler {
    pub fn new(request_body: Body, producer: ResponseProducer) -> Self {
        Self {
            request_body,
            producer,
        }
    }
}

impl curl::easy::Handler for CurlHandler {
    // Gets called by curl for each line of data in the HTTP request header.
    fn header(&mut self, data: &[u8]) -> bool {
        // Curl calls this function for all lines in the response not part of the response body, not just for headers.
        // We need to inspect the contents of the string in order to determine what it is and how to parse it, just as
        // if we were reading from the socket of a HTTP/1.0 or HTTP/1.1 connection ourselves.

        // Is this the status line?
        if let Some((version, status)) = parse::parse_status_line(data) {
            // self.version = Some(version);
            // self.status_code = Some(status);
            return true;
        }

        // Is this a header line?
        if let Some((name, value)) = parse::parse_header(data) {
            // self.headers.insert(name, value);
            return true;
        }

        // Is this the end of the response header?
        if data == b"\r\n" {
            // self.finalize_headers();
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    // Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        // Don't bother if the request is canceled.
        if self.producer.is_closed() {
            return Err(curl::easy::ReadError::Abort);
        }

        self.request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    // Gets called by curl when it wants to seek to a certain position in the request body.
    fn seek(&mut self, whence: io::SeekFrom) -> curl::easy::SeekResult {
        curl::easy::SeekResult::CantSeek
    }

    // Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        log::trace!("received {} bytes of data", data.len());

        if self.producer.is_closed() {
            log::debug!("aborting write, request is already closed");
            return Ok(0);
        }

        // let mut buffer = self.state.buffer.lock().unwrap();

        // // If there is existing data in the buffer, pause the request until the existing data is consumed.
        // if !buffer.is_empty() {
        //     trace!("response buffer is not empty, pausing transfer");
        //     return Err(curl::easy::WriteError::Pause);
        // }

        // // Store the data in the buffer.
        // *buffer = Bytes::from(data);

        // // Notify the reader.
        // self.state.read_waker.wake();

        // Ok(buffer.len())
        unimplemented!()
    }

    // Gets called by curl whenever it wishes to log a debug message.
    fn debug(&mut self, kind: InfoType, data: &[u8]) {
        match kind {
            InfoType::Text => log::trace!("{}", String::from_utf8_lossy(data).trim_end()),
            InfoType::HeaderIn | InfoType::DataIn => log::trace!(target: "chttp::wire", "<< {}", format_byte_string(data)),
            InfoType::HeaderOut | InfoType::DataOut => log::trace!(target: "chttp::wire", ">> {}", format_byte_string(data)),
            _ => (),
        }
    }
}

struct CurlResponseStream {
    state: Arc<State>,
    // buffer_future: Option<MutexLockFuture>,
}

impl AsyncRead for CurlResponseStream {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context, dest: &mut [u8]) -> Poll<Result<usize, io::Error>> {
        log::trace!("received read request for {} bytes", dest.len());

        if dest.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // Set the current read waker.
        self.state.read_waker.register(cx.waker());

        // Attempt to read some from the buffer.
        // let mut buffer = self.state.buffer.lock().unwrap();

        // If the request failed, return an error.
        // if let Some(error) = self.state.error.borrow() {
        //     log::debug!("failing read due to error: {:?}", error);
        //     return Err(error.clone().into());
        // }

        // // If data is available, read some.
        // if !buffer.is_empty() {
        //     let amount_to_consume = dest.len().min(buffer.len());
        //     log::trace!("read buffer contains {} bytes, consuming {} bytes", buffer.len(), amount_to_consume);

        //     let consumed = buffer.split_to(amount_to_consume);
        //     (&mut dest[0..amount_to_consume]).copy_from_slice(&consumed);

        //     return Ok(Async::Ready(consumed.len()));
        // }

        // // If the request is closed, return EOF.
        // if self.state.is_closed() {
        //     log::trace!("request is closed, satisfying read request with EOF");
        //     return Ok(Async::Ready(0));
        // }

        // // Before we yield, ensure the request is not paused so that the buffer may be filled with new data.
        // if let Some(agent) = self.state.agent.borrow() {
        //     if let Some(token) = self.state.token.get() {
        //         agent.unpause_write(token)?;
        //     }
        // }

        log::trace!("buffer is empty, read is pending");
        Poll::Pending
    }
}
