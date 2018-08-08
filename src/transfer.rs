use body::Body;
use curl;
use curl::easy::InfoType;
use error::Error;
use futures::sync::oneshot;
use std::sync::mpsc;
use http::{self, Response};
use log;
use std::io::{self, Read, Write};
use std::str::{self, FromStr};
use stream;

pub struct CurlTransfer {
    pub(crate) easy: curl::easy::Easy2<TransferState>,
    pub(crate) future: Option<oneshot::Receiver<Result<Response<stream::StreamReader>, Error>>>,
}

impl CurlTransfer {
    pub fn new() -> Result<Self, Error> {
        let headers_channel = oneshot::channel();
        let response_body_stream = stream::Stream::new().split();

        let mut easy = curl::easy::Easy2::new(TransferState {
            request_body: Body::Empty,
            response: http::response::Builder::new(),
            response_body_reader: Some(response_body_stream.0),
            response_body_writer: response_body_stream.1,
            future: Some(headers_channel.0),
        });

        easy.verbose(log_enabled!(log::Level::Trace))?;
        easy.signal(false)?;

        Ok(Self {
            easy: easy,
            future: Some(headers_channel.1),
        })
    }

    pub fn request_body_mut(&mut self) -> &mut Body {
        &mut self.easy.get_mut().request_body
    }
}

/// Sends and receives data between libcurl and the outside world.
pub struct TransferState {
    /// The request body to be sent.
    request_body: Body,

    /// Builder for the response object.
    response: http::response::Builder,

    response_body_reader: Option<stream::StreamReader>,

    /// Output channel for the response body.
    response_body_writer: stream::StreamWriter,

    future: Option<oneshot::Sender<Result<Response<stream::StreamReader>, Error>>>,
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
        if let Some(sender) = self.future.take() {
            sender.send(Err(error)).is_ok();
        } else {

        }
    }

    fn complete(&mut self) {
        let response = self.response.body(self.response_body_reader.take().unwrap()).unwrap();

        self.future.take()
            .unwrap()
            .send(Ok(response))
            .is_ok();
    }
}

impl curl::easy::Handler for TransferState {
    // Gets called by libcurl for each line of data in the HTTP request header.
    fn header(&mut self, data: &[u8]) -> bool {
        let line = match str::from_utf8(data) {
            Ok(s) => s,
            _  => return false,
        };

        // libcurl calls this function for all lines in the response not part of the response body, not just for headers.
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

    // Gets called by libcurl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        self.request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    // Gets called by libcurl when it wants to seek to a certain position in the request body.
    fn seek(&mut self, whence: io::SeekFrom) -> curl::easy::SeekResult {
        if self.request_body.is_seekable() {
            unimplemented!();
        } else {
            curl::easy::SeekResult::CantSeek
        }
    }

    // Gets called by libcurl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        match self.response_body_writer.write_all(data) {
            Ok(()) => Ok(data.len()),
            Err(_) => Ok(0),
        }
    }

    // Gets called by libcurl whenever it wishes to log a debug message.
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

pub struct TransferBody {
    stream: stream::StreamReader,
    error_rx: mpsc::Receiver<Error>,
}

impl Read for TransferBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Ok(error) = self.error_rx.try_recv() {
            return Err(error.into());
        }

        self.stream.read(buf)
    }
}
