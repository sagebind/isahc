use curl;
use http;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::io::Read;
use std::mem;
use std::rc::Rc;
use std::str;
use std::str::FromStr;
use std::time::Duration;
use super::*;


const WAIT_TIMEOUT_MS: u64 = 1000;

/// Sets various protocol and connection options for a transport.
#[derive(Clone, Debug)]
pub struct Options {
    pub follow_redirects: bool,
    pub max_redirects: Option<u32>,
    pub preferred_http_version: Option<http::Version>,
    pub timeout: Option<Duration>,
    pub connect_timeout: Duration,
    pub tcp_keepalive: Option<Duration>,
    pub tcp_nodelay: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            follow_redirects: false,
            max_redirects: None,
            preferred_http_version: None,
            timeout: None,
            connect_timeout: Duration::from_secs(300),
            tcp_keepalive: None,
            tcp_nodelay: false,
        }
    }
}


/// A low-level reusable HTTP client with a single connection pool.
///
/// Transports are stateful objects that can only perform one request at a time.
pub struct Transport {
    /// A curl multi handle used to execute transfers. Also holds the internal connection pool.
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
    buffer: VecDeque<u8>,
}

impl Transport {
    /// Create a new transport.
    ///
    /// Initializing a transport is an expensive operation, so be sure to re-use your transports instead of discarding
    /// and recrreating them.
    pub fn new() -> Transport {
        Transport::with_options(Options::default())
    }

    /// Create a new transport with the given options.
    pub fn with_options(options: Options) -> Transport {
        let data = Rc::new(RefCell::new(Data {
            request_body: Body::default(),
            response: http::response::Builder::new(),
            header_complete: false,
            buffer: VecDeque::new(),
        }));

        Transport {
            multi: curl::multi::Multi::new(),
            handle: None,
            options: options,
            data: data,
        }
    }

    /// Check if the transport is ready to begin a new request.
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

    /// Send a request.
    pub fn send(&mut self, request: Request) -> Result<http::response::Builder, Error> {
        self.prepare(request)?;

        // Wait for the headers to be read.
        while !self.data.borrow().header_complete {
            if self.multi.perform()? > 0 {
                self.multi.wait(&mut [], Duration::from_millis(WAIT_TIMEOUT_MS))?;
            }
        }

        Ok(mem::replace(&mut self.data.borrow_mut().response, http::response::Builder::new()))
    }

    /// Prepare a request.
    fn prepare(&mut self, request: Request) -> Result<(), Error> {
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
        if self.options.follow_redirects {
            easy.follow_location(true)?;
            if let Some(max) = self.options.max_redirects {
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
    fn finish(&mut self) -> Result<(), Error> {
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
}

impl Read for Transport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut pos = 0;

        while pos < buffer.len() {
            if let Some(byte) = self.data.borrow_mut().buffer.pop_front() {
                buffer[pos] = byte;
                pos += 1;
                continue;
            }

            if !self.is_active() {
                break;
            }

            self.multi.wait(&mut [], Duration::from_millis(WAIT_TIMEOUT_MS)).unwrap();

            match self.multi.perform() {
                // No more transfers are active.
                Ok(0) => {
                    self.finish().unwrap();
                }
                // Success, but transfer is incomplete.
                Ok(_) => {
                    continue;
                }
                // Error during transfer.
                Err(e) => {
                    println!("{:?}", e);
                }
            }
        }

        Ok(pos)
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

    fn read(&mut self, data: &mut [u8]) -> Result<usize, curl::easy::ReadError> {
        self.data.borrow_mut()
            .request_body
            .read(data)
            .map_err(|_| curl::easy::ReadError::Abort)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        self.data.borrow_mut().buffer.extend(data);
        Ok(data.len())
    }
}
