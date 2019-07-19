use crate::{parse, Body, Error};
use curl::easy::{InfoType, ReadError, SeekResult, WriteError};
use futures::channel::oneshot;
use futures::prelude::*;
use http::Response;
use sluice::pipe;
use std::ascii;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::*;

/// Manages the state of a single request/response life cycle.
///
/// During the lifetime of a handler, it will receive callbacks from curl about
/// the progress of the request, and the handler will incrementally build up a
/// response struct as the response is received.
///
/// Every request handler has an associated `Future` that can be used to poll
/// the state of the response. The handler will complete the future once the
/// final HTTP response headers are received. The body of the response (if any)
/// is made available to the consumer of the future, and is also driven by the
/// request handler until the response body is fully consumed or discarded.
///
/// If dropped before the response is finished, the associated future will be
/// completed with an `Aborted` error.
pub(crate) struct RequestHandler {
    /// The ID of the request that this handler is managing. Assigned by the
    /// request agent.
    id: Option<usize>,

    /// Sender for the associated future.
    sender: Option<oneshot::Sender<Result<http::response::Builder, Error>>>,

    /// The body to be sent in the request.
    request_body: Body,

    /// A waker used with reading the request body asynchronously. Populated by
    /// an agent when the request is initialized.
    request_body_waker: Option<Waker>,

    /// Status code of the response.
    response_status_code: Option<http::StatusCode>,

    /// HTTP version of the response.
    response_version: Option<http::Version>,

    /// Response headers received so far.
    response_headers: http::HeaderMap,

    /// Writing end of the pipe where the response body is written.
    response_body_writer: pipe::PipeWriter,

    /// A waker used with writing the response body asynchronously. Populated by
    /// an agent when the request is initialized.
    response_body_waker: Option<Waker>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ResponseState {
    Active,
    Canceled,
    Completed,
}

impl RequestHandler {
    /// Create a new request handler and an associated response future.
    pub(crate) fn new(request_body: Body) -> (Self, RequestHandlerFuture) {
        let (sender, receiver) = oneshot::channel();
        let (response_body_reader, response_body_writer) = pipe::pipe();

        (
            Self {
                id: None,
                sender: Some(sender),
                request_body,
                request_body_waker: None,
                response_status_code: None,
                response_version: None,
                response_headers: http::HeaderMap::new(),
                response_body_writer,
                response_body_waker: None,
            },
            RequestHandlerFuture {
                receiver,
                response_body_reader: Some(response_body_reader),
            },
        )
    }

    /// Get the current state of the handler.
    pub(crate) fn state(&self) -> ResponseState {
        match self.sender.as_ref() {
            Some(sender) => {
                if sender.is_canceled() {
                    ResponseState::Canceled
                } else {
                    ResponseState::Active
                }
            }
            None => ResponseState::Completed,
        }
    }

    /// Initialize the handler and prepare it for the request to begin.
    ///
    /// This is called from within the agent thread when it registers the
    /// request handled by this handler with the multi handle and begins the
    /// request's execution.
    pub(crate) fn init(&mut self, id: usize, request_waker: Waker, response_waker: Waker) {
        // Init should not be called more than once.
        debug_assert!(self.id.is_none());
        debug_assert!(self.request_body_waker.is_none());
        debug_assert!(self.response_body_waker.is_none());

        log::debug!("initializing handler for request [id={}]", id);
        self.id = Some(id);
        self.request_body_waker = Some(request_waker);
        self.response_body_waker = Some(response_waker);
    }

    /// Finishes building the response with a given body and completes the
    /// associated future with the response.
    pub(crate) fn complete(&mut self) {
        if let Some(sender) = self.sender.take() {
            let mut builder = http::Response::builder();

            if let Some(status) = self.response_status_code.take() {
                builder.status(status);
            }

            if let Some(version) = self.response_version.take() {
                builder.version(version);
            }

            for (name, values) in self.response_headers.drain() {
                for value in values {
                    builder.header(&name, value);
                }
            }

            match sender.send(Ok(builder)) {
                Ok(()) => log::debug!("request completed [id={:?}]", self.id),
                Err(_) => log::debug!("request canceled by user [id={:?}]", self.id),
            }
        }
    }

    /// Complete the associated future with an error.
    pub(crate) fn complete_with_error(&mut self, error: impl Into<Error>) {
        if let Some(sender) = self.sender.take() {
            match sender.send(Err(error.into())) {
                Ok(()) => log::warn!("request completed with error [id={:?}]", self.id),
                Err(_) => log::debug!("request canceled by user [id={:?}]", self.id),
            }
        }
    }
}

impl curl::easy::Handler for RequestHandler {
    /// Gets called by curl for each line of data in the HTTP response header.
    fn header(&mut self, data: &[u8]) -> bool {
        // Don't bother if the request is canceled.
        if self.state() != ResponseState::Active {
            return false;
        }

        // Curl calls this function for all lines in the response not part of
        // the response body, not just for headers. We need to inspect the
        // contents of the string in order to determine what it is and how to
        // parse it, just as if we were reading from the socket of a HTTP/1.0 or
        // HTTP/1.1 connection ourselves.

        // Is this the status line?
        if let Some((version, status)) = parse::parse_status_line(data) {
            self.response_version = Some(version);
            self.response_status_code = Some(status);

            // Also clear any pre-existing headers that might be left over from
            // a previous intermediate response.
            self.response_headers.clear();

            return true;
        }

        // Is this a header line?
        if let Some((name, value)) = parse::parse_header(data) {
            self.response_headers.insert(name, value);
            return true;
        }

        // Is this the end of the response header?
        if data == b"\r\n" {
            // We will acknowledge the end of the header, but we can't complete
            // our response future yet. If curl decides to follow a redirect,
            // then this current response is not the final response and not the
            // one we should complete with.
            //
            // Instead, we will complete the future when curl marks the transfer
            // as complete, or when we start receiving a response body.
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    /// Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, ReadError> {
        // Don't bother if the request is canceled.
        if self.state() != ResponseState::Active {
            return Err(ReadError::Abort);
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
                }
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
            io::SeekFrom::Start(0) => {
                if self.request_body.reset() {
                    SeekResult::Ok
                } else {
                    SeekResult::CantSeek
                }
            }
            // We can't do any other type of seek, sorry :(
            _ => SeekResult::CantSeek,
        }
    }

    /// Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        log::trace!("received {} bytes of data", data.len());

        // Don't bother if the request is canceled.
        if self.state() == ResponseState::Canceled {
            log::debug!("aborting write, request was canceled");
            return Ok(0);
        }

        // Now that we've started receiving the response body, we know no more
        // redirects can happen and we can complete the future safely.
        self.complete();

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
                }
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
            String::from_utf8(
                bytes
                    .as_ref()
                    .iter()
                    .flat_map(|byte| ascii::escape_default(*byte))
                    .collect(),
            )
            .unwrap_or_else(|_| String::from("<binary>"))
        }

        match kind {
            InfoType::Text => {
                log::debug!(target: "chttp::curl", "{}", String::from_utf8_lossy(data).trim_end())
            }
            InfoType::HeaderIn | InfoType::DataIn => {
                log::trace!(target: "chttp::wire", "<< {}", format_byte_string(data))
            }
            InfoType::HeaderOut | InfoType::DataOut => {
                log::trace!(target: "chttp::wire", ">> {}", format_byte_string(data))
            }
            _ => (),
        }
    }
}

impl fmt::Debug for RequestHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestHandler({:?})", self.id)
    }
}

// A future for a response produced by a request handler.
#[derive(Debug)]
pub(crate) struct RequestHandlerFuture {
    receiver: oneshot::Receiver<Result<http::response::Builder, Error>>,

    /// Reading end of the pipe where the response body is written.
    ///
    /// This is moved out of the future and set as the response body stream when
    /// the future is ready. We continue to stream the response body from the
    /// handler.
    response_body_reader: Option<pipe::PipeReader>,
}

impl Future for RequestHandlerFuture {
    type Output = Result<Response<Body>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = Pin::new(&mut self.receiver);

        match inner.poll(cx) {
            // Response headers are still pending.
            Poll::Pending => Poll::Pending,

            // Response headers have been fully received.
            Poll::Ready(Ok(Ok(mut builder))) => {
                // Since we only take the reader here, we are allowed to panic
                // if someone tries to poll us again after the end of this call.
                let reader = self.response_body_reader.take().unwrap();

                // If a Content-Length header is present, include that
                // information in the body as well.
                let content_length = builder
                    .headers_ref()
                    .unwrap()
                    .get(http::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse().ok());

                let body = match content_length {
                    Some(len) => Body::reader_sized(reader, len),
                    None => Body::reader(reader),
                };

                Poll::Ready(match builder.body(body) {
                    Ok(response) => Ok(response),
                    Err(e) => Err(Error::InvalidHttpFormat(e)),
                })
            }

            // The request handler produced an error.
            Poll::Ready(Ok(Err(e))) => Poll::Ready(Err(e)),

            // The request handler was dropped abnormally.
            Poll::Ready(Err(oneshot::Canceled)) => Poll::Ready(Err(Error::Aborted)),
        }
    }
}

impl Drop for RequestHandlerFuture {
    fn drop(&mut self) {
        self.receiver.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>() {}

    #[test]
    fn traits() {
        is_send::<RequestHandlerFuture>();
    }
}
