use crate::{parse, Body, Error};
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use curl::easy::{InfoType, ReadError, SeekResult, WriteError};
use futures_io::{AsyncRead, AsyncWrite};
use futures_util::task::AtomicWaker;
use http::Response;
use sluice::pipe;
use std::ascii;
use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

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
    sender: Option<Sender<Result<http::response::Builder, Error>>>,

    /// State shared by the handler and its future.
    shared: Arc<Shared>,

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

/// State shared by the handler and its future.
#[derive(Debug, Default)]
struct Shared {
    /// A waker used by the handler to wake up the associated future.
    waker: AtomicWaker,
}

impl RequestHandler {
    /// Create a new request handler and an associated response future.
    pub(crate) fn new(request_body: Body) -> (Self, RequestHandlerFuture) {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let shared = Arc::new(Shared::default());
        let (response_body_reader, response_body_writer) = pipe::pipe();

        (
            Self {
                id: None,
                sender: Some(sender),
                shared: shared.clone(),
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
                shared,
                response_body_reader: Some(response_body_reader),
            },
        )
    }

    /// Determine if the associated future has been dropped.
    fn is_disconnected(&self) -> bool {
        Arc::strong_count(&self.shared) == 1
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

    /// Handle a result produced by curl for this handler's current transfer.
    pub(crate) fn on_result(&mut self, result: Result<(), curl::Error>) {
        match result {
            Ok(()) => self.flush_response_headers(),
            Err(e) => {
                log::debug!("curl error: {}", e);
                self.complete(Err(e.into()));
            }
        }
    }

    /// Mark the future as completed successfully with the response headers
    /// received so far.
    fn flush_response_headers(&mut self) {
        if self.sender.is_some() {
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

            self.complete(Ok(builder));
        }
    }

    /// Complete the associated future with a result.
    fn complete(&mut self, result: Result<http::response::Builder, Error>) {
        if let Some(sender) = self.sender.take() {
            if let Err(e) = result.as_ref() {
                log::warn!("request completed with error [id={:?}]: {}", self.id, e);
            }

            match sender.send(result) {
                Ok(()) => {
                    self.shared.waker.wake();
                }
                Err(_) => {
                    log::debug!("request canceled by user [id={:?}]", self.id);
                }
            }
        }
    }
}

impl curl::easy::Handler for RequestHandler {
    /// Gets called by curl for each line of data in the HTTP response header.
    fn header(&mut self, data: &[u8]) -> bool {
        // Abort the request if it has been canceled.
        if self.is_disconnected() {
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
        // Abort the request if it has been canceled.
        if self.is_disconnected() {
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
        // If curl wants to seek to the beginning, there's a chance that we
        // can do that.
        if whence == io::SeekFrom::Start(0) && self.request_body.reset() {
            SeekResult::Ok
        } else {
            log::warn!("seek requested for request body, but it is not supported");
            // We can't do any other type of seek, sorry :(
            SeekResult::CantSeek
        }
    }

    /// Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        log::trace!("received {} bytes of data", data.len());

        // Now that we've started receiving the response body, we know no more
        // redirects can happen and we can complete the future safely.
        self.flush_response_headers();

        // Create a task context using a waker provided by the agent so we can
        // do an asynchronous write.
        if let Some(waker) = self.response_body_waker.as_ref() {
            let mut context = Context::from_waker(waker);

            match Pin::new(&mut self.response_body_writer).poll_write(&mut context, data) {
                Poll::Pending => Err(WriteError::Pause),
                Poll::Ready(Ok(len)) => Ok(len),
                Poll::Ready(Err(e)) => {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        log::warn!(
                            "failed to write response body because the response reader was dropped"
                        );
                    } else {
                        log::error!("error writing response body to buffer: {}", e);
                    }
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
                log::debug!(target: "isahc::curl", "{}", String::from_utf8_lossy(data).trim_end())
            }
            InfoType::HeaderIn | InfoType::DataIn => {
                log::trace!(target: "isahc::wire", "<< {}", format_byte_string(data))
            }
            InfoType::HeaderOut | InfoType::DataOut => {
                log::trace!(target: "isahc::wire", ">> {}", format_byte_string(data))
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
    /// Receiving end of a channel that the handler sends its result over.
    receiver: Receiver<Result<http::response::Builder, Error>>,

    /// State shared by the handler and its future.
    shared: Arc<Shared>,

    /// Reading end of the pipe where the response body is written.
    ///
    /// This is moved out of the future and set as the response body stream when
    /// the future is ready. We continue to stream the response body from the
    /// handler.
    response_body_reader: Option<pipe::PipeReader>,
}

impl RequestHandlerFuture {
    pub(crate) fn join(mut self) -> Result<Response<Body>, Error> {
        match self.receiver.recv() {
            Ok(Ok(builder)) => self.complete(builder),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::Aborted),
        }
    }

    fn complete(&mut self, mut builder: http::response::Builder) -> Result<Response<Body>, Error> {
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

        match builder.body(body) {
            Ok(response) => Ok(response),
            Err(e) => Err(Error::InvalidHttpFormat(e)),
        }
    }
}

impl Future for RequestHandlerFuture {
    type Output = Result<Response<Body>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.shared.waker.register(cx.waker());

        match self.receiver.try_recv() {
            // Response headers are still pending.
            Err(TryRecvError::Empty) => Poll::Pending,

            // Response headers have been fully received.
            Ok(Ok(builder)) => Poll::Ready(self.complete(builder)),

            // The request handler produced an error.
            Ok(Err(e)) => Poll::Ready(Err(e)),

            // The request handler was dropped abnormally.
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(Error::Aborted)),
        }
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
