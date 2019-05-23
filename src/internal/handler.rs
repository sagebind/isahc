use crate::body::Body;
use crate::internal::format_byte_string;
use crate::internal::parse;
use crate::internal::response::ResponseProducer;
use curl::easy::{ReadError, InfoType, WriteError};
use futures::prelude::*;
use futures::task::AtomicWaker;
use sluice::pipe;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::*;

/// Drives the state for a single request/response life cycle.
pub struct CurlHandler {
    /// The body to be sent in the request.
    request_body: Body,

    /// Pipe to write the response body to.
    response_body_writer: pipe::PipeWriter,

    producer: ResponseProducer,
}

impl CurlHandler {
    pub fn new(request_body: Body, producer: ResponseProducer) -> Self {
        Self {
            request_body,
            response_body_writer: pipe::pipe().1,
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
            self.producer.version = Some(version);
            self.producer.status_code = Some(status);
            return true;
        }

        // Is this a header line?
        if let Some((name, value)) = parse::parse_header(data) {
            // self.producer.headers.insert(name, value);
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
    fn read(&mut self, data: &mut [u8]) -> Result<usize, ReadError> {
        // Don't bother if the request is canceled.
        if self.producer.is_closed() {
            return Err(curl::easy::ReadError::Abort);
        }

        // TODO: Custom context + waker that unpauses read.

        match self.request_body.poll_read(0, data) {
            Poll::Pending => Err(ReadError::Pause),
            Poll::Ready(Ok(len)) => Ok(len),
            Poll::Ready(Err(e)) => {
                log::error!("error reading request body: {}", e);
                Err(ReadError::Abort)
            },
        }
    }

    // Gets called by curl when it wants to seek to a certain position in the request body.
    fn seek(&mut self, whence: io::SeekFrom) -> curl::easy::SeekResult {
        curl::easy::SeekResult::CantSeek
    }

    // Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        log::trace!("received {} bytes of data", data.len());

        if self.producer.is_closed() {
            log::debug!("aborting write, request is already closed");
            return Ok(0);
        }

        // TODO: Custom context + waker that unpauses write.

        match self.response_body_writer.poll_write(0, data) {
            Poll::Pending => Err(WriteError::Pause),
            Poll::Ready(Ok(len)) => Ok(len),
            Poll::Ready(Err(e)) => {
                log::error!("error writing response body to buffer: {}", e);
                Ok(0)
            },
        }
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

struct AgentWaker {}

struct RequestWaker {
    inner: AgentWaker,
    token: usize,
}
