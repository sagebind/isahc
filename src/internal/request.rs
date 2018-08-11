use body::Body;
use curl;
use curl::easy::InfoType;
use error::Error;
use futures::Future;
use futures::sync::oneshot;
use http::{self, Request, Response};
use log;
use options::*;
use ringtail::io::*;
use std::io::{self, Read, Write};
use std::str::{self, FromStr};
use std::sync::mpsc;

/// Create a new curl request.
pub fn create(request: Request<Body>, options: &Options) -> Result<(CurlRequest, impl Future<Item=Response<Body>, Error=Error>), Error> {
    let (future_tx, future_rx) = oneshot::channel();
    let (error_tx, error_rx) = mpsc::channel();
    let (request_parts, request_body) = request.into_parts();
    let (response_reader, response_writer) = PipeBuilder::default()
        .capacity(options.buffer_size)
        .build();

    let mut easy = curl::easy::Easy2::new(TransferState {
        future: Some(future_tx),
        errors: error_tx,
        request_body: request_body,
        response: http::response::Builder::new(),
        response_sink: response_writer,
        response_stream: Some(CurlResponseStream {
            reader: response_reader,
            errors: error_rx,
        }),
    });

    easy.verbose(log_enabled!(log::Level::Trace))?;
    easy.signal(false)?;

    if let Some(timeout) = options.timeout {
        easy.timeout(timeout)?;
    }

    easy.connect_timeout(options.connect_timeout)?;

    easy.tcp_nodelay(options.tcp_nodelay)?;
    if let Some(interval) = options.tcp_keepalive {
        easy.tcp_keepalive(true)?;
        easy.tcp_keepintvl(interval)?;
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
    if let Some(version) = options.preferred_http_version {
        easy.http_version(match version {
            http::Version::HTTP_10 => curl::easy::HttpVersion::V10,
            http::Version::HTTP_11 => curl::easy::HttpVersion::V11,
            http::Version::HTTP_2 => curl::easy::HttpVersion::V2,
            _ => curl::easy::HttpVersion::Any,
        })?;
    }

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

    // If the request body is non-empty, tell curl that we are going to upload something.
    if !easy.get_ref().request_body.is_empty() {
        easy.upload(true)?;
    }

    let future_rx = future_rx.then(|result| match result {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Err(e),
        Err(oneshot::Canceled) => Err(Error::Canceled),
    }).map(|response| {
        response.map(|body| {
            Body::from_reader(body)
        })
    });

    Ok((CurlRequest(easy), future_rx))
}

pub struct CurlRequest(pub curl::easy::Easy2<TransferState>);

/// Provides a stream of the response body for an ongoing request.
pub struct CurlResponseStream {
    reader: PipeReader,
    errors: mpsc::Receiver<Error>,
}

impl Read for CurlResponseStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Ok(error) = self.errors.try_recv() {
            return Err(error.into());
        }

        self.reader.read(buf)
    }
}

/// Sends and receives data between curl and the outside world.
pub struct TransferState {
    /// Future to resolve when the initial request is complete.
    future: Option<oneshot::Sender<Result<Response<CurlResponseStream>, Error>>>,

    /// Channel where errors are sent after the future has already been resolved.
    errors: mpsc::Sender<Error>,

    /// The request body to be sent.
    request_body: Body,

    /// Builder for the response object.
    response: http::response::Builder,

    /// Sink where the response body is written to.
    response_sink: PipeWriter,

    /// The response body stream.
    response_stream: Option<CurlResponseStream>,
}

impl TransferState {
    pub fn is_canceled(&self) -> bool {
        match self.future {
            Some(ref future) => future.is_canceled(),
            None => false,
        }
    }

    /// Fail the transfer with the given error.
    pub fn fail(&mut self, error: Error) {
        // If the future has not been completed yet, complete it with the given error. Otherwise send it to the error
        // channel for the response stream to handle.
        if let Some(future) = self.future.take() {
            if future.send(Err(error)).is_err() {
                panic!("error resolving future, must be dropped");
            }
        } else {
            self.errors.send(error).unwrap();
        }
    }

    fn complete(&mut self) {
        let response = self.response.body(self.response_stream.take().unwrap()).unwrap();

        self.future.take()
            .unwrap()
            .send(Ok(response))
            .is_ok();
    }
}

impl curl::easy::Handler for TransferState {
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
            self.complete();
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    // Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        self.request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    // Gets called by curl when it wants to seek to a certain position in the request body.
    fn seek(&mut self, _whence: io::SeekFrom) -> curl::easy::SeekResult {
        if self.request_body.is_seekable() {
            unimplemented!();
        } else {
            curl::easy::SeekResult::CantSeek
        }
    }

    // Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        match self.response_sink.write_all(data) {
            Ok(()) => Ok(data.len()),
            Err(_) => Ok(0),
        }
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
