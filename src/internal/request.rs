use body::Body;
use bytes::Bytes;
use curl;
use curl::easy::InfoType;
use error::Error;
use futures::channel::oneshot;
use futures::executor;
use futures::prelude::*;
use http::{self, Request, Response};
use lazycell::AtomicLazyCell;
use log;
use options::*;
use std::io::{self, Read};
use std::mem;
use std::sync::atomic::*;
use std::sync::{Arc, Mutex};
use super::agent;
use super::format_byte_string;
use super::parse;

const STATUS_READY: usize = 0;
const STATUS_CLOSED: usize = 1;

/// Create a new curl request.
pub fn create<B: Into<Body>>(request: Request<B>, options: &Options) -> Result<(CurlRequest, impl Future<Item=Response<Body>, Error=Error>), Error> {
    // Set up the plumbing...
    let (future_tx, future_rx) = oneshot::channel();
    let (request_parts, request_body) = request.into_parts();

    let mut easy = curl::easy::Easy2::new(CurlHandler {
        state: Arc::new(RequestState::new(options.clone())),
        future: Some(future_tx),
        request_body: request_body.into(),
        version: None,
        status_code: None,
        headers: http::HeaderMap::default(),
    });

    easy.verbose(log_enabled!(log::Level::Trace))?;
    easy.signal(false)?;
    easy.buffer_size(options.buffer_size)?;

    if let Some(timeout) = options.timeout {
        easy.timeout(timeout)?;
    }

    easy.connect_timeout(options.connect_timeout)?;

    easy.tcp_nodelay(options.tcp_nodelay)?;
    if let Some(interval) = options.tcp_keepalive {
        easy.tcp_keepalive(true)?;
        easy.tcp_keepintvl(interval)?;
    } else {
        easy.tcp_keepalive(false)?;
    }

    match options.redirect_policy {
        RedirectPolicy::None => {
            easy.follow_location(false)?;
        }
        RedirectPolicy::Follow => {
            easy.follow_location(true)?;
        }
        RedirectPolicy::Limit(max) => {
            easy.follow_location(true)?;
            easy.max_redirections(max)?;
        }
    }

    if let Some(limit) = options.max_upload_speed {
        easy.max_send_speed(limit)?;
    }

    if let Some(limit) = options.max_download_speed {
        easy.max_recv_speed(limit)?;
    }

    // Set a preferred HTTP version to negotiate.
    easy.http_version(match options.preferred_http_version {
        Some(http::Version::HTTP_10) => curl::easy::HttpVersion::V10,
        Some(http::Version::HTTP_11) => curl::easy::HttpVersion::V11,
        Some(http::Version::HTTP_2) => curl::easy::HttpVersion::V2,
        _ => curl::easy::HttpVersion::Any,
    })?;

    if let Some(ref proxy) = options.proxy {
        easy.proxy(&format!("{}", proxy))?;
    }

    // Set the request data according to the request given.
    easy.custom_request(request_parts.method.as_str())?;
    easy.url(&request_parts.uri.to_string())?;

    let mut headers = curl::easy::List::new();
    for (name, value) in request_parts.headers.iter() {
        let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
        headers.append(&header)?;
    }
    easy.http_headers(headers)?;

    // Enable automatic response decompression.
    easy.accept_encoding("")?;

    // If the request body is non-empty, tell curl that we are going to upload something.
    if !easy.get_ref().request_body.is_empty() {
        easy.upload(true)?;

        if let Some(len) = easy.get_ref().request_body.len() {
            // If we know the size of the request body up front, tell curl about it.
            easy.in_filesize(len as u64)?;
        }
    }

    let future_rx = future_rx.then(|result| match result {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Err(e),
        Err(oneshot::Canceled) => {
            error!("request canceled by agent; this should never happen!");
            Err(Error::Canceled)
        },
    }).map(|response| response.map(Body::from_reader));

    Ok((CurlRequest(easy), future_rx))
}

/// Encapsulates a curl request that can be executed by an agent.
#[derive(Debug)]
pub struct CurlRequest(pub curl::easy::Easy2<CurlHandler>);

/// Sends and receives data between curl and the outside world.
#[derive(Debug)]
pub struct CurlHandler {
    /// Shared request state.
    state: Arc<RequestState>,

    /// Future that resolves when the response headers are received.
    future: Option<oneshot::Sender<Result<Response<CurlResponseStream>, Error>>>,

    /// A request body to send.
    request_body: Body,

    /// Status code of the response.
    status_code: Option<http::StatusCode>,

    /// HTTP version of the response.
    version: Option<http::Version>,

    /// Response headers received so far.
    headers: http::HeaderMap,
}

impl CurlHandler {
    /// Mark the request as completed successfully.
    pub fn complete(&mut self) {
        self.ensure_future_is_completed();
        self.state.close();
        self.state.read_waker.wake();
    }

    /// Fail the request with the given error.
    pub fn fail(&mut self, error: curl::Error) {
        self.state.error.fill(error).is_ok();

        // If the future has not been completed yet, complete it with the given error.
        if let Some(future) = self.future.take() {
            let error = self.state.error.borrow().unwrap().clone().into();

            if future.send(Err(error)).is_err() {
                debug!("future was canceled, canceling the request");
                if let Some(agent) = self.state.agent.borrow() {
                    if let Some(token) = self.state.token.get() {
                        agent.cancel_request(token).is_ok();
                    }
                }
            }
        }

        self.state.close();
        self.state.read_waker.wake();
    }

    pub fn set_agent(&self, agent: agent::Handle) {
        if self.state.agent.fill(agent).is_err() {
            warn!("request agent cannot be changed once set");
        }
    }

    pub fn set_token(&self, token: usize) {
        if self.state.token.fill(token).is_err() {
            warn!("request token cannot be changed once set");
        }
    }

    fn is_canceled(&self) -> bool {
        match self.future {
            Some(ref future) => future.is_canceled(),
            None => false,
        }
    }

    /// Determine if curl is about to perform a redirect.
    fn is_about_to_redirect(&self) -> bool {
        self.state.options.redirect_policy != RedirectPolicy::None
            && self.status_code.filter(http::StatusCode::is_redirection).is_some()
            && self.headers.contains_key("Location")
    }

    /// Completes the associated future when headers have been received.
    fn finalize_headers(&mut self) {
        if self.is_about_to_redirect() {
            debug!("preparing for redirect to {:?}", self.headers.get("Location"));

            // It appears that curl will do a redirect, so instead of completing the future, just reset the response
            // state.
            self.status_code = None;
            self.version = None;
            self.headers.clear();

            return;
        }

        self.ensure_future_is_completed();
    }

    fn ensure_future_is_completed(&mut self) {
        if let Some(future) = self.future.take() {
            let body = CurlResponseStream {
                state: self.state.clone(),
            };

            let mut builder = http::Response::builder();
            builder.status(self.status_code.take().unwrap());
            builder.version(self.version.take().unwrap());

            for (name, values) in self.headers.drain() {
                for value in values {
                    builder.header(&name, value);
                }
            }

            let response = builder
                .body(body)
                .unwrap();

            future.send(Ok(response)).is_ok();
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
            self.version = Some(version);
            self.status_code = Some(status);
            return true;
        }

        // Is this a header line?
        if let Some((name, value)) = parse::parse_header(data) {
            self.headers.insert(name, value);
            return true;
        }

        // Is this the end of the response header?
        if data == b"\r\n" {
            self.finalize_headers();
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    // Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        // Don't bother if the request is canceled.
        if self.is_canceled() {
            return Err(curl::easy::ReadError::Abort);
        }

        self.request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    // Gets called by curl when it wants to seek to a certain position in the request body.
    fn seek(&mut self, _whence: io::SeekFrom) -> curl::easy::SeekResult {
        curl::easy::SeekResult::CantSeek
    }

    // Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        trace!("received {} bytes of data", data.len());

        if self.state.is_closed() {
            debug!("aborting write, request is already closed");
            return Ok(0);
        }

        let mut buffer = self.state.buffer.lock().unwrap();

        // If there is existing data in the buffer, pause the request until the existing data is consumed.
        if !buffer.is_empty() {
            trace!("response buffer is not empty, pausing transfer");
            return Err(curl::easy::WriteError::Pause);
        }

        // Store the data in the buffer.
        *buffer = Bytes::from(data);

        // Notify the reader.
        self.state.read_waker.wake();

        Ok(buffer.len())
    }

    // Gets called by curl whenever it wishes to log a debug message.
    fn debug(&mut self, kind: InfoType, data: &[u8]) {
        match kind {
            InfoType::Text => trace!("{}", String::from_utf8_lossy(data).trim_right()),
            InfoType::HeaderIn | InfoType::DataIn => trace!(target: "chttp::wire", "<< {}", format_byte_string(data)),
            InfoType::HeaderOut | InfoType::DataOut => trace!(target: "chttp::wire", ">> {}", format_byte_string(data)),
            _ => (),
        }
    }
}

/// Provides an asynchronous stream of the response body for an ongoing request.
#[derive(Debug)]
pub struct CurlResponseStream {
    state: Arc<RequestState>,
}

// Synchronous wrapper around async stream.
impl io::Read for CurlResponseStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        executor::block_on(AsyncReadExt::read(self, buf).map(|r| r.2))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        executor::block_on(AsyncReadExt::read_exact(self, buf).map(|_| ()))
    }

    fn read_to_end(&mut self, dest: &mut Vec<u8>) -> io::Result<usize> {
        let mut buf = Vec::new();
        mem::swap(&mut buf, dest);

        match executor::block_on(AsyncReadExt::read_to_end(self, buf)) {
            Ok((_, buf)) => {
                *dest = buf;
                Ok(dest.len())
            },
            Err(e) => Err(e),
        }
    }
}

impl AsyncRead for CurlResponseStream {
    fn poll_read(&mut self, cx: &mut task::Context, dest: &mut [u8]) -> Result<Async<usize>, io::Error> {
        trace!("received read request for {} bytes", dest.len());

        if dest.is_empty() {
            return Ok(Async::Ready(0));
        }

        // Set the current read waker.
        self.state.read_waker.register(cx.waker());

        // Attempt to read some from the buffer.
        let mut buffer = self.state.buffer.lock().unwrap();

        // If the request failed, return an error.
        if let Some(error) = self.state.error.borrow() {
            debug!("failing read due to error: {:?}", error);
            return Err(error.clone().into());
        }

        // If data is available, read some.
        if !buffer.is_empty() {
            let amount_to_consume = dest.len().min(buffer.len());
            trace!("read buffer contains {} bytes, consuming {} bytes", buffer.len(), amount_to_consume);

            let consumed = buffer.split_to(amount_to_consume);
            (&mut dest[0..amount_to_consume]).copy_from_slice(&consumed);

            return Ok(Async::Ready(consumed.len()));
        }

        // If the request is closed, return EOF.
        if self.state.is_closed() {
            trace!("request is closed, satisfying read request with EOF");
            return Ok(Async::Ready(0));
        }

        // Before we yield, ensure the request is not paused so that the buffer may be filled with new data.
        if let Some(agent) = self.state.agent.borrow() {
            if let Some(token) = self.state.token.get() {
                agent.unpause_write(token)?;
            }
        }

        trace!("buffer is empty, read is pending");
        Ok(Async::Pending)
    }
}

/// Holds the shared state of a request.
#[derive(Debug)]
struct RequestState {
    options: Options,
    status: AtomicUsize,
    agent: AtomicLazyCell<agent::Handle>,
    token: AtomicLazyCell<usize>,
    error: AtomicLazyCell<curl::Error>,
    buffer: Mutex<Bytes>,
    read_waker: task::AtomicWaker,
}

impl RequestState {
    fn new(options: Options) -> Self {
        Self {
            options: options,
            status: AtomicUsize::new(STATUS_READY),
            agent: AtomicLazyCell::new(),
            token: AtomicLazyCell::new(),
            error: AtomicLazyCell::new(),
            buffer: Mutex::new(Bytes::new()),
            read_waker: task::AtomicWaker::default(),
        }
    }

    fn is_closed(&self) -> bool {
        self.status.load(Ordering::SeqCst) == STATUS_CLOSED
    }

    fn close(&self) {
        self.status.store(STATUS_CLOSED, Ordering::SeqCst);
    }
}
