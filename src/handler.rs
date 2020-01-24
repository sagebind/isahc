use crate::{parse, response::EffectiveUri, Body, Error, Metrics};
use crossbeam_utils::atomic::AtomicCell;
use curl::easy::{InfoType, ReadError, SeekResult, WriteError};
use curl_sys::CURL;
use futures_channel::oneshot::Sender;
use futures_io::{AsyncRead, AsyncWrite};
use futures_util::{pin_mut, task::AtomicWaker};
use http::{Response, Uri};
use sluice::pipe;
use std::{
    ascii,
    ffi::CStr,
    fmt,
    future::Future,
    io,
    os::raw::c_char,
    pin::Pin,
    ptr,
    sync::Arc,
    task::{Context, Poll, Waker},
};

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
    /// State shared by the handler and its future.
    shared: Arc<Shared>,

    /// Sender for the associated future.
    sender: Option<Sender<Result<http::response::Builder, Error>>>,

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

    /// Metrics object for publishing metrics data to. Lazily initialized.
    metrics: Option<Metrics>,

    /// Raw pointer to the associated curl easy handle. The pointer is not owned
    /// by this struct, but the parent struct to this one, so we know it will be
    /// valid at least for the lifetime of this struct (assuming all other
    /// invariants are upheld).
    handle: *mut CURL,
}

// Would be send implicitly except for the raw CURL pointer.
#[allow(unsafe_code)]
unsafe impl Send for RequestHandler {}

/// State shared by the handler and its future.
///
/// This is also used to keep track of the lifetime of the request.
#[derive(Debug)]
struct Shared {
    /// The ID of the request that this handler is managing. Assigned by the
    /// request agent.
    id: AtomicCell<usize>,

    /// A waker used by the handler to wake up the associated future.
    waker: AtomicWaker,

    completed: AtomicCell<bool>,
    response_body_dropped: AtomicCell<bool>,
}

impl RequestHandler {
    /// Create a new request handler and an associated response future.
    pub(crate) fn new(
        request_body: Body,
    ) -> (
        Self,
        impl Future<Output = Result<Response<ResponseBodyReader>, Error>>,
    ) {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let shared = Arc::new(Shared {
            id: AtomicCell::new(usize::max_value()),
            waker: AtomicWaker::default(),
            completed: AtomicCell::new(false),
            response_body_dropped: AtomicCell::new(false),
        });
        let (response_body_reader, response_body_writer) = pipe::pipe();

        let handler = Self {
            sender: Some(sender),
            shared: shared.clone(),
            request_body,
            request_body_waker: None,
            response_status_code: None,
            response_version: None,
            response_headers: http::HeaderMap::new(),
            response_body_writer,
            response_body_waker: None,
            metrics: None,
            handle: ptr::null_mut(),
        };

        // Create a future that resolves when the handler receives the response
        // headers.
        let future = async move {
            let builder = receiver.await.map_err(|_| Error::Aborted)??;

            let reader = ResponseBodyReader {
                inner: response_body_reader,
                shared,
            };

            builder.body(reader).map_err(Error::InvalidHttpFormat)
        };

        (handler, future)
    }

    fn is_future_canceled(&self) -> bool {
        self.sender
            .as_ref()
            .map(Sender::is_canceled)
            .unwrap_or(false)
    }

    /// Initialize the handler and prepare it for the request to begin.
    ///
    /// This is called from within the agent thread when it registers the
    /// request handled by this handler with the multi handle and begins the
    /// request's execution.
    pub(crate) fn init(
        &mut self,
        id: usize,
        handle: *mut CURL,
        request_waker: Waker,
        response_waker: Waker,
    ) {
        // Init should not be called more than once.
        debug_assert!(self.shared.id.load() == usize::max_value());
        debug_assert!(self.request_body_waker.is_none());
        debug_assert!(self.response_body_waker.is_none());

        log::debug!("initializing handler for request [id={}]", id);
        self.shared.id.store(id);
        self.handle = handle;
        self.request_body_waker = Some(request_waker);
        self.response_body_waker = Some(response_waker);
    }

    /// Handle a result produced by curl for this handler's current transfer.
    pub(crate) fn on_result(&mut self, result: Result<(), curl::Error>) {
        self.shared.completed.store(true);

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
                builder = builder.status(status);
            }

            if let Some(version) = self.response_version.take() {
                builder = builder.version(version);
            }

            if let Some(headers) = builder.headers_mut() {
                headers.extend(self.response_headers.drain());
            }

            if let Some(uri) = self.get_effective_uri() {
                builder = builder.extension(EffectiveUri(uri));
            }

            // Include metrics in response, but only if it was created. If
            // metrics are disabled then it won't have been created.
            if let Some(metrics) = self.metrics.clone() {
                builder = builder.extension(metrics);
            }

            self.complete(Ok(builder));
        }
    }

    /// Complete the associated future with a result.
    fn complete(&mut self, result: Result<http::response::Builder, Error>) {
        if let Some(sender) = self.sender.take() {
            if let Err(e) = result.as_ref() {
                log::warn!(
                    "request completed with error [id={:?}]: {}",
                    self.shared.id,
                    e
                );
            }

            match sender.send(result) {
                Ok(()) => {
                    self.shared.waker.wake();
                }
                Err(_) => {
                    log::debug!("request canceled by user [id={:?}]", self.shared.id);
                }
            }
        }
    }

    #[allow(unsafe_code)]
    fn get_effective_uri(&mut self) -> Option<Uri> {
        Some(self.handle)
            .filter(|ptr| !ptr.is_null())
            .and_then(|handle| unsafe {
                let mut ptr = ptr::null::<c_char>();

                if curl_sys::curl_easy_getinfo(handle, curl_sys::CURLINFO_EFFECTIVE_URL, &mut ptr)
                    != curl_sys::CURLE_OK
                {
                    None
                } else {
                    Some(ptr)
                }
            })
            .filter(|ptr| !ptr.is_null())
            .map(|ptr| unsafe { CStr::from_ptr(ptr) })
            .and_then(|cstr| cstr.to_str().ok())
            .and_then(|s| s.parse().ok())
    }
}

impl curl::easy::Handler for RequestHandler {
    /// Gets called by curl for each line of data in the HTTP response header.
    fn header(&mut self, data: &[u8]) -> bool {
        // Abort the request if it has been canceled.
        if self.is_future_canceled() {
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
            self.response_headers.append(name, value);
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
        if self.is_future_canceled() {
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

        // Abort the request if it has been canceled.
        if self.shared.response_body_dropped.load() {
            return Ok(0);
        }

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

    /// Capture transfer progress updates from curl.
    #[allow(unsafe_code)]
    fn progress(&mut self, dltotal: f64, dlnow: f64, ultotal: f64, ulnow: f64) -> bool {
        // Initialize metrics if required.
        let metrics = self.metrics.get_or_insert_with(Metrics::new);

        // Store the progress values given.
        metrics.inner.upload_progress.store(ulnow);
        metrics.inner.upload_total.store(ultotal);
        metrics.inner.download_progress.store(dlnow);
        metrics.inner.download_total.store(dltotal);

        // Also scrape additional metrics.
        if !self.handle.is_null() {
            unsafe {
                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_SPEED_UPLOAD,
                    metrics.inner.upload_speed.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_SPEED_DOWNLOAD,
                    metrics.inner.download_speed.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_NAMELOOKUP_TIME,
                    metrics.inner.namelookup_time.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_CONNECT_TIME,
                    metrics.inner.connect_time.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_APPCONNECT_TIME,
                    metrics.inner.appconnect_time.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_PRETRANSFER_TIME,
                    metrics.inner.pretransfer_time.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_STARTTRANSFER_TIME,
                    metrics.inner.starttransfer_time.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_TOTAL_TIME,
                    metrics.inner.total_time.as_ptr(),
                );

                curl_sys::curl_easy_getinfo(
                    self.handle,
                    curl_sys::CURLINFO_REDIRECT_TIME,
                    metrics.inner.redirect_time.as_ptr(),
                );
            }
        }

        true
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
        write!(f, "RequestHandler({:?})", self.shared.id)
    }
}

/// Wrapper around a pipe reader that returns an error that tracks transfer
/// cancellation.
pub(crate) struct ResponseBodyReader {
    inner: pipe::PipeReader,
    shared: Arc<Shared>,
}

impl AsyncRead for ResponseBodyReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let inner = &mut self.inner;
        pin_mut!(inner);

        match inner.poll_read(cx, buf) {
            // On EOF, check to see if the transfer was cancelled, and if so,
            // return an error.
            Poll::Ready(Ok(0)) => {
                if !self.shared.completed.load() {
                    Poll::Ready(Err(io::ErrorKind::ConnectionAborted.into()))
                } else {
                    Poll::Ready(Ok(0))
                }
            }
            poll => poll,
        }
    }
}

impl Drop for ResponseBodyReader {
    fn drop(&mut self) {
        self.shared.response_body_dropped.store(true);
    }
}
