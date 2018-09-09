use body::Body;
use bytes::Bytes;
use curl;
use curl::easy::InfoType;
use error::Error;
use futures::{
    channel::oneshot,
    executor,
    prelude::*,
};
use http::{self, Request, Response};
use lazycell::AtomicLazyCell;
use log;
use options::*;
use std::{
    io,
    io::Read,
    mem,
    str,
    str::FromStr,
    sync::{
        atomic::*,
        Arc,
        Mutex,
    },
};
use super::agent;

const STATUS_READY: usize = 0;
const STATUS_CLOSED: usize = 1;

/// Create a new curl request.
pub fn create<B: Into<Body>>(request: Request<B>, default_options: &Options) -> Result<(CurlRequest, impl Future<Item=Response<Body>, Error=Error>), Error> {
    // Set up the plumbing...
    let (future_tx, future_rx) = oneshot::channel();
    let (request_parts, request_body) = request.into_parts();

    let mut easy = curl::easy::Easy2::new(CurlHandler {
        state: Arc::new(RequestState::default()),
        future: Some(future_tx),
        request_body: request_body.into(),
        response: http::response::Builder::new(),
    });

    // If the request has options attached, use those options, otherwise use the default options.
    let options = request_parts.extensions.get().unwrap_or(default_options);

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

    if let Some(limit) = options.max_upload_speed {
        easy.max_send_speed(limit)?;
    }

    if let Some(limit) = options.max_download_speed {
        easy.max_recv_speed(limit)?;
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
pub struct CurlRequest(pub curl::easy::Easy2<CurlHandler>);

/// Sends and receives data between curl and the outside world.
pub struct CurlHandler {
    state: Arc<RequestState>,
    future: Option<oneshot::Sender<Result<Response<CurlResponseStream>, Error>>>,
    request_body: Body,
    response: http::response::Builder,
}

impl CurlHandler {
    /// Mark the request as completed successfully.
    pub fn complete(&self) {
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

    /// Completes the associated future when headers have been received.
    fn finalize_headers(&mut self) -> bool {
        if let Some(future) = self.future.take() {
            let body = CurlResponseStream {
                state: self.state.clone(),
            };

            let response = self.response.body(body).unwrap();

            future.send(Ok(response)).is_ok()
        } else {
            warn!("headers already finalized");
            false
        }
    }
}

impl curl::easy::Handler for CurlHandler {
    // Gets called by curl for each line of data in the HTTP request header.
    fn header(&mut self, data: &[u8]) -> bool {
        let line = match str::from_utf8(data) {
            Ok(s) => s,
            _  => return false,
        };

        // curl calls this function for all lines in the response not part of the response body, not just for headers.
        // We need to inspect the contents of the string in order to determine what it is and how to parse it, just as
        // if we were reading from the socket of a HTTP/1.0 or HTTP/1.1 connection ourselves.

        // Is this the status line?
        if line.starts_with("HTTP/") {
            // Parse the HTTP protocol version.
            let version = match &line[0..8] {
                "HTTP/2.0" => http::Version::HTTP_2,
                "HTTP/1.1" => http::Version::HTTP_11,
                "HTTP/1.0" => http::Version::HTTP_10,
                "HTTP/0.9" => http::Version::HTTP_09,
                _ => http::Version::default(),
            };
            self.response.version(version);

            // Parse the status code.
            let status_code = match http::StatusCode::from_str(&line[9..12]) {
                Ok(s) => s,
                _ => return false,
            };
            self.response.status(status_code);

            return true;
        }

        // Is this a header line?
        if let Some(pos) = line.find(":") {
            let (name, value) = line.split_at(pos);
            let value = value[2..].trim();
            self.response.header(name, value);

            return true;
        }

        // Is this the end of the response header?
        if line == "\r\n" {
            return self.finalize_headers();
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
        fn format_byte_string(bytes: &[u8]) -> String {
            use std::ascii;

            String::from_utf8(bytes
                .iter()
                .flat_map(|byte| ascii::escape_default(*byte))
                .collect()).unwrap()
        }

        match kind {
            InfoType::Text => trace!("{}", String::from_utf8_lossy(data).trim_right()),
            InfoType::HeaderIn | InfoType::DataIn => trace!(target: "chttp::wire", "<< {}", format_byte_string(data)),
            InfoType::HeaderOut | InfoType::DataOut => trace!(target: "chttp::wire", ">> {}", format_byte_string(data)),
            _ => (),
        }
    }
}

/// Provides an asynchronous stream of the response body for an ongoing request.
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
struct RequestState {
    status: AtomicUsize,
    agent: AtomicLazyCell<agent::Handle>,
    token: AtomicLazyCell<usize>,
    error: AtomicLazyCell<curl::Error>,
    buffer: Mutex<Bytes>,
    read_waker: task::AtomicWaker,
}

impl Default for RequestState {
    fn default() -> Self {
        Self {
            status: AtomicUsize::new(STATUS_READY),
            agent: AtomicLazyCell::new(),
            token: AtomicLazyCell::new(),
            error: AtomicLazyCell::new(),
            buffer: Mutex::new(Bytes::new()),
            read_waker: task::AtomicWaker::default(),
        }
    }
}

impl RequestState {
    fn is_closed(&self) -> bool {
        self.status.load(Ordering::SeqCst) == STATUS_CLOSED
    }

    fn close(&self) {
        self.status.store(STATUS_CLOSED, Ordering::SeqCst);
    }
}
