use crate::body::Body;
use super::parse;
use super::response::ResponseProducer;
use curl::easy::{ReadError, InfoType, WriteError, SeekResult};
use futures::prelude::*;
use sluice::pipe;
use std::ascii;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::*;

/// Drives the state for a single request/response life cycle.
pub struct CurlHandler {
    /// The ID of the request that this handler is managing. Assigned by the
    /// request agent.
    id: Option<usize>,

    /// The body to be sent in the request.
    request_body: Body,

    /// A waker used with reading the request body asynchronously. Populated by
    /// the agent when the request is initialized.
    request_body_waker: Option<Waker>,

    /// Reading end of the pipe where the response body is written.
    ///
    /// This is moved out of the handler and set as the response body stream
    /// when the response future is ready. We continue to stream the response
    /// body using the writer.
    response_body_reader: Option<pipe::PipeReader>,

    /// Writing end of the pipe where the response body is written.
    response_body_writer: pipe::PipeWriter,

    /// A waker used with writing the response body asynchronously. Populated by
    /// the agent when the request is initialized.
    response_body_waker: Option<Waker>,

    producer: ResponseProducer,
}

impl CurlHandler {
    pub fn new(request_body: Body, producer: ResponseProducer) -> Self {
        let (response_body_reader, response_body_writer) = pipe::pipe();

        Self {
            id: None,
            request_body,
            request_body_waker: None,
            response_body_reader: Some(response_body_reader),
            response_body_writer,
            response_body_waker: None,
            producer,
        }
    }

    /// Initialize the handler and prepare it for the request to begin.
    ///
    /// This is called from within the agent thread when it registers the
    /// request handled by this handler with the multi handle and begins the
    /// request's execution.
    pub fn init(&mut self, id: usize, request_waker: Waker, response_waker: Waker) {
        // Init should not be called more than once.
        debug_assert!(self.id.is_none());
        debug_assert!(self.request_body_waker.is_none());
        debug_assert!(self.response_body_waker.is_none());

        log::debug!("initializing handler for request [id={}]", id);
        self.id = Some(id);
        self.request_body_waker = Some(request_waker);
        self.response_body_waker = Some(response_waker);
    }

    fn finish_response_and_complete(&mut self) {
        if let Some(body) = self.response_body_reader.take() {
            // TODO: Extract and include Content-Length here.
            self.producer.finish(Body::reader(body));
        } else {
            log::debug!("response already finished!");
        }
    }
}

impl curl::easy::Handler for CurlHandler {
    /// Gets called by curl for each line of data in the HTTP request header.
    fn header(&mut self, data: &[u8]) -> bool {
        // Curl calls this function for all lines in the response not part of
        // the response body, not just for headers. We need to inspect the
        // contents of the string in order to determine what it is and how to
        // parse it, just as if we were reading from the socket of a HTTP/1.0 or
        // HTTP/1.1 connection ourselves.

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
            self.finish_response_and_complete();
            // self.finalize_headers();
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    /// Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, ReadError> {
        // Don't bother if the request is canceled.
        if self.producer.is_closed() {
            return Err(curl::easy::ReadError::Abort);
        }

        // Create a task context using a waker provided by the agent so we can
        // do an asynchronous read.
        if let Some(waker) = self.request_body_waker.as_ref() {
            let mut context = Context::from_waker(waker);

            match Pin::new(&mut self.request_body).poll_read(&mut context, data) {
                Poll::Pending => Err(ReadError::Pause),
                Poll::Ready(Ok(len)) => Ok(len),
                Poll::Ready(Err(e)) => {
                    log::error!("error reading request body: {}", e);
                    Err(ReadError::Abort)
                },
            }
        } else {
            // The request should never be started without calling init first.
            log::error!("request has not been initialized!");
            Err(ReadError::Abort)
        }
    }

    /// Gets called by curl when it wants to seek to a certain position in the
    /// request body.
    ///
    /// Since this method is synchronous and provides no means of deferring the
    /// seek, we can't do any async operations in this callback. That's why we
    /// only support trivial types of seeking.
    fn seek(&mut self, whence: io::SeekFrom) -> SeekResult {
        match whence {
            // If curl wants to seek to the beginning, there's a chance that we
            // can do that.
            io::SeekFrom::Start(0) => if self.request_body.reset() {
                SeekResult::Ok
            } else {
                SeekResult::CantSeek
            },
            // We can't do any other type of seek, sorry :(
            _ => SeekResult::CantSeek,
        }
    }

    /// Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        log::trace!("received {} bytes of data", data.len());

        if self.producer.is_closed() {
            log::debug!("aborting write, request is already closed");
            return Ok(0);
        }

        // Create a task context using a waker provided by the agent so we can
        // do an asynchronous write.
        if let Some(waker) = self.response_body_waker.as_ref() {
            let mut context = Context::from_waker(waker);

            match Pin::new(&mut self.response_body_writer).poll_write(&mut context, data) {
                Poll::Pending => Err(WriteError::Pause),
                Poll::Ready(Ok(len)) => Ok(len),
                Poll::Ready(Err(e)) => {
                    log::error!("error writing response body to buffer: {}", e);
                    Ok(0)
                },
            }
        } else {
            // The request should never be started without calling init first.
            log::error!("request has not been initialized!");
            Ok(0)
        }
    }

    /// Gets called by curl whenever it wishes to log a debug message.
    ///
    /// Since we're using the log crate, this callback normalizes the debug info
    /// and writes it to our log.
    fn debug(&mut self, kind: InfoType, data: &[u8]) {
        fn format_byte_string(bytes: impl AsRef<[u8]>) -> String {
            String::from_utf8(bytes
                .as_ref()
                .iter()
                .flat_map(|byte| ascii::escape_default(*byte))
                .collect())
                .unwrap_or(String::from("<binary>"))
        }

        match kind {
            InfoType::Text => log::trace!("{}", String::from_utf8_lossy(data).trim_end()),
            InfoType::HeaderIn | InfoType::DataIn => log::trace!(target: "chttp::wire", "<< {}", format_byte_string(data)),
            InfoType::HeaderOut | InfoType::DataOut => log::trace!(target: "chttp::wire", ">> {}", format_byte_string(data)),
            _ => (),
        }
    }
}

impl Drop for CurlHandler {
    fn drop(&mut self) {
        // Ensure we always at least attempt to complete the associated response
        // future before the handler is closed.
        self.finish_response_and_complete();
    }
}

impl fmt::Debug for CurlHandler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CurlHandler({:?})", self.id)
    }
}
