use curl;
use http;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::io::Read;
use std::rc::Rc;
use std::str;
use std::str::FromStr;
use std::time::Duration;
use super::{Error, Entity, Request, Response};


/// A low-level reusable HTTP client with a single connection pool.
///
/// Transports are stateful objects that can only perform one request at a time.
pub struct Transport {
    /// A curl multi handle used to execute transfers. Also holds the internal connection pool.
    multi: curl::multi::Multi,
    /// A curl easy handle for configuring requests. Lazily initialized.
    handle: Option<Handle>,
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
    request_body: Entity,
    /// Builder for the response object.
    response: http::response::Builder,
    /// Temporary buffer for the response body.
    buffer: VecDeque<u8>,
}

impl Transport {
    /// Create a new transport.
    ///
    /// Initializing a transport is an expensive operation, so be sure to re-use your transports instead of discarding
    /// and recrreating them.
    pub fn new() -> Transport {
        let data = Rc::new(RefCell::new(Data {
            request_body: Entity::Empty,
            response: http::response::Builder::new(),
            buffer: VecDeque::new(),
        }));

        Transport {
            multi: curl::multi::Multi::new(),
            handle: None,
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

    /// Begin executing a request.
    pub fn begin(&mut self, request: Request) -> Result<(), Error> {
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

        // Disable signal handling.
        easy.signal(false).unwrap();

        // Set request method.
        easy.custom_request(request.method().as_str()).unwrap();

        // Set the URL.
        let url = format!("{}", request.uri());
        easy.url(&url).unwrap();

        // Set headers.
        let mut headers = curl::easy::List::new();
        for (name, value) in request.headers() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header).unwrap();
        }
        easy.http_headers(headers).unwrap();

        // Set the request body.
        self.data.borrow_mut().request_body = request.into_parts().1;

        // Finalize the easy handle state and attach it to the multi handle to be executed.
        let easy = self.multi.add2(easy).unwrap();
        self.handle = Some(Handle::Active(easy));

        // Reset buffers and other temporary data.
        self.data.borrow_mut().buffer.clear();

        Ok(())
    }

    pub fn read_response(&mut self) {}

    /// Reset the transport to the ready state.
    ///
    /// Note that this will abort the current request.
    fn finish(&mut self) {
        // Reset the curl easy handle.
        self.handle = match self.handle.take() {
            Some(Handle::Active(easy)) => {
                let easy = self.multi.remove2(easy).unwrap();
                Some(Handle::Ready(easy))
            },
            handle => handle,
        };
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

            self.multi.wait(&mut [], Duration::from_secs(1)).unwrap();

            match self.multi.perform() {
                // No more transfers are active.
                Ok(0) => {
                    self.finish();
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
    /// Called by curl when a response header line is read.
    fn header(&mut self, data: &[u8]) -> bool {
        let line = str::from_utf8(data).unwrap();

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
            let status_code = http::StatusCode::from_str(&line[9..12]).unwrap();
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
            // self.data.borrow_mut().response_header_read = true;
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    /// Called by curl when a chunk of the response body is read.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        self.data.borrow_mut().buffer.extend(data);
        Ok(data.len())
    }
}
