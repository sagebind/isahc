use buffer::Buffer;
use curl;
use http;
use std::cell::RefCell;
use std::io;
use std::io::Read;
use std::mem;
use std::rc::Rc;
use std::str;
use std::str::FromStr;
use std::time::Duration;
use super::*;


const DEFAULT_TIMEOUT_MS: u64 = 1000;


/// A low-level reusable HTTP client with a single connection.
///
/// Transports are stateful objects that can only perform one request at a time.
pub struct Transport {
    /// A curl multi handle used to execute transfers.
    multi: curl::multi::Multi,
    /// A curl easy handle for configuring requests. Lazily initialized.
    handle: Option<Handle>,
    /// Protocol and connection options.
    options: Options,
    /// Contains the current request and response data.
    data: Rc<RefCell<Data>>,
}

/// Wrapper for the various states of a curl easy handle.
enum Handle {
    Ready(curl::easy::Easy2<Collector>),
    Active(curl::multi::Easy2Handle<Collector>),
}

struct Data {
    /// Request body to be sent.
    request_body: Body,
    /// Builder for the response object.
    response: http::response::Builder,
    /// Indicates if the header has been read completely.
    header_complete: bool,
    /// Temporary buffer for the response body.
    buffer: Buffer,
}

impl Transport {
    /// Create a new transport.
    ///
    /// Initializing a transport is an expensive operation, so be sure to re-use your transports instead of discarding
    /// and recreating them.
    pub fn new() -> Transport {
        Transport::with_options(Options::default())
    }

    /// Create a new transport with the given options.
    pub fn with_options(options: Options) -> Transport {
        let data = Rc::new(RefCell::new(Data {
            request_body: Body::default(),
            response: http::response::Builder::new(),
            header_complete: false,
            buffer: Buffer::new(),
        }));

        Transport {
            multi: curl::multi::Multi::new(),
            handle: None,
            options: options,
            data: data,
        }
    }

    /// Check if the transport is ready to execute a new request.
    #[inline]
    pub fn is_ready(&self) -> bool {
        !self.is_active()
    }

    /// Check if the transport is already executing a request.
    #[inline]
    pub fn is_active(&self) -> bool {
        if let Some(Handle::Active(_)) = self.handle {
            true
        } else {
            false
        }
    }

    /// Execute a new request.
    ///
    /// If another transfer is already underway, `Error::TransportBusy` will be returned.
    pub fn execute(&mut self, request: Request) -> Result<http::response::Builder, Error> {
        self.begin_request(request)?;

        // Wait for the headers to be read.
        while !self.data.borrow().header_complete {
            self.dispatch()?;
        }

        Ok(mem::replace(&mut self.data.borrow_mut().response, http::response::Builder::new()))
    }

    /// Cancel the current request.
    ///
    /// Returns `true` if the request was canceled, or `false` if there was no active request to cancel.
    pub fn cancel(&mut self) -> Result<bool, Error> {
        if self.is_active() {
            // Reset the curl handle.
            self.end_request()?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Begin a new request.
    fn begin_request(&mut self, request: Request) -> Result<(), Error> {
        // Prepare the easy handle.
        let mut easy = match self.handle.take() {
            // We're already engaged in a different request.
            Some(Handle::Active(_)) => {
                return Err(Error::TransportBusy);
            }
            // Re-use an already created handle.
            Some(Handle::Ready(mut easy)) => {
                easy.reset();
                easy
            }
            // Initialize a new handle.
            None => {
                curl::easy::Easy2::new(Collector {
                    data: self.data.clone(),
                })
            }
        };

        easy.max_connects(1)?;
        easy.signal(false)?;

        // Configure connection based on our options struct.
        if let Some(timeout) = self.options.timeout {
            easy.timeout(timeout)?;
        }
        easy.connect_timeout(self.options.connect_timeout)?;
        easy.tcp_nodelay(self.options.tcp_nodelay)?;
        if let Some(interval) = self.options.tcp_keepalive {
            easy.tcp_keepalive(true)?;
            easy.tcp_keepintvl(interval)?;
        }

        // Configure redirects.
        match self.options.redirect_policy {
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
        if let Some(version) = self.options.preferred_http_version {
            easy.http_version(match version {
                http::Version::HTTP_10 => curl::easy::HttpVersion::V10,
                http::Version::HTTP_11 => curl::easy::HttpVersion::V11,
                http::Version::HTTP_2 => curl::easy::HttpVersion::V2,
                _ => curl::easy::HttpVersion::Any,
            })?;
        }

        // Set a proxy to use.
        if let Some(ref proxy) = self.options.proxy {
            easy.proxy(&format!("{}", proxy))?;
        }

        // Set the request data according to the request given.
        easy.custom_request(request.method().as_str())?;
        easy.url(&format!("{}", request.uri()))?;

        let mut headers = curl::easy::List::new();
        for (name, value) in request.headers() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header)?;
        }
        easy.http_headers(headers)?;

        // Set the request body.
        let body = request.into_parts().1;
        if !body.is_empty() {
            easy.upload(true)?;
        }
        self.data.borrow_mut().request_body = body;

        // Finalize the easy handle state and attach it to the multi handle to be executed.
        let easy = self.multi.add2(easy)?;
        self.handle = Some(Handle::Active(easy));

        // Reset buffers and other temporary data.
        self.data.borrow_mut().header_complete = false;
        self.data.borrow_mut().buffer.clear();

        Ok(())
    }

    /// Reset the transport to the ready state.
    ///
    /// Note that this will abort the current request.
    fn end_request(&mut self) -> Result<(), Error> {
        // Reset the curl easy handle.
        self.handle = match self.handle.take() {
            Some(Handle::Active(easy)) => {
                let easy = self.multi.remove2(easy)?;
                Some(Handle::Ready(easy))
            },
            handle => handle,
        };

        Ok(())
    }

    /// Dispatch reads and writes, blocking the current thread if necessary.
    fn dispatch(&mut self) -> Result<(), Error> {
        if self.is_active() {
            // Determine the blocking timeout value.
            let timeout = self.multi.get_timeout()?.unwrap_or(Duration::from_millis(DEFAULT_TIMEOUT_MS));

            // Block until activity is detected or the timeout passes.
            self.multi.wait(&mut [], timeout)?;

            // Perform any pending reads or writes. If `perform()` returns zero, then the current transfer is complete.
            if self.multi.perform()? == 0 {
                self.end_request()?;

                // The current transfer has stopped, but that does not mean it succeeded. Check the transfer status now
                // and return any errors we find.
                let mut result = None;

                self.multi.messages(|message| {
                    if let Some(Err(e)) = message.result() {
                        result = Some(e);
                    }
                });

                if let Some(e) = result {
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }
}

impl Read for Transport {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        // Block until bytes arrive in the buffer or the transfer is complete.
        while self.data.borrow().buffer.is_empty() && self.is_active() {
            // Attempt to fill the buffer with more bytes.
            self.dispatch()?;
        }

        // Copy bytes from the internal buffer to the given one.
        self.data.borrow_mut().buffer.read(dst)
    }
}

// Even though we use Rc within the transport, all Rc references live within the Transport struct, so it is safe to move
// the transport as a whole between threads.
unsafe impl Send for Transport {}


/// Receives callbacks from curl and incrementally constructs a response.
struct Collector {
    data: Rc<RefCell<Data>>,
}

impl curl::easy::Handler for Collector {
    // Gets called by curl for each line of data in the HTTP request header.
    fn header(&mut self, data: &[u8]) -> bool {
        let line = match str::from_utf8(data) {
            Ok(s) => s,
            _  => return false,
        };

        // Curl calls this function for all lines in the response not part of the response body, not just for headers.
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
            self.data.borrow_mut().response.version(version);

            // Parse the status code.
            let status_code = match http::StatusCode::from_str(&line[9..12]) {
                Ok(s) => s,
                _ => return false,
            };
            self.data.borrow_mut().response.status(status_code);

            return true;
        }

        // Is this a header line?
        if let Some(pos) = line.find(":") {
            let (name, value) = line.split_at(pos);
            let value = value[2..].trim();
            self.data.borrow_mut().response.header(name, value);

            return true;
        }

        // Is this the end of the response header?
        if line == "\r\n" {
            self.data.borrow_mut().header_complete = true;
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    // Gets called by curl when attempting to send bytes of the request body.
    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        self.data.borrow_mut()
            .request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    // Gets called by curl when bytes from the response body are received.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        self.data.borrow_mut().buffer.push(data);
        Ok(data.len())
    }
}
