use curl;
use http;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::str;
use std::str::FromStr;


/// Manages the state of an active HTTP request.
///
/// Transfer objects cannot be reused for multiple requests.
pub struct Transfer {
    state: Rc<RefCell<State>>,
}

/// Encapsulates the state of a single transfer.
struct State {
    /// Indicates if the transfer is complete.
    complete: bool,

    /// The request being sent.
    request: ::Request,

    /// Builder for the response object.
    response: Option<http::response::Builder>,

    /// Indicates if we are finished with reading the response header.
    response_header_read: bool,

    /// Temporary buffer for the response body.
    buffer: VecDeque<u8>,
}

impl Transfer {
    /// Create a new transfer for the given request.
    pub fn new(request: ::Request) -> Transfer {
        // Set up the transfer state.
        let state = Rc::new(RefCell::new(State {
            complete: false,
            request: request,
            response: Some(http::response::Builder::new()),
            response_header_read: false,
            buffer: VecDeque::new(),
        }));

        Transfer {
            state: state,
        }
    }

    /// Create a new curl easy handle for this transfer.
    pub fn create_handle(&self) -> curl::easy::Easy2<Collector> {
        // Create a new curl easy handle.
        let mut easy_handle = curl::easy::Easy2::new(Collector {
            state: self.state.clone(),
        });

        // Get the request for configuring the easy handle with.
        let request = &self.state.borrow().request;

        // Set request method.
        easy_handle.custom_request(request.method().as_str()).unwrap();

        // Set the URL.
        let url = format!("{}", request.uri());
        easy_handle.url(&url).unwrap();

        // Set headers.
        let mut headers = curl::easy::List::new();
        for (name, value) in request.headers() {
            let header = format!("{}: {}", name.as_str(), value.to_str().unwrap());
            headers.append(&header).unwrap();
        }
        easy_handle.http_headers(headers).unwrap();

        easy_handle.signal(false).unwrap();

        easy_handle
    }

    pub fn take_response_builder(&self) -> http::response::Builder {
        self.state.borrow_mut().response.take().unwrap()
    }

    pub fn is_response_header_read(&self) -> bool {
        self.state.borrow().response_header_read
    }

    /// Check if the transfer is complete.
    pub fn is_complete(&self) -> bool {
        self.state.borrow().complete
    }

    /// Mark the request as complete.
    pub(crate) fn complete(&self) {
        self.state.borrow_mut().complete = true;
    }

    /// Read a byte from the response body buffer.
    pub fn buffer_read(&self) -> Option<u8> {
        self.state.borrow_mut().buffer.pop_front()
    }
}


/// Receives callbacks from cURL and incrementally constructs the response.
pub struct Collector {
    state: Rc<RefCell<State>>,
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
            self.state.borrow_mut().response.as_mut().unwrap().version(version);

            // Parse the status code.
            let status_code = http::StatusCode::from_str(&line[9..12]).unwrap();
            self.state.borrow_mut().response.as_mut().unwrap().status(status_code);

            return true;
        }

        // Is this a header line?
        if let Some(pos) = line.find(":") {
            let (name, value) = line.split_at(pos);
            let value = value[2..].trim();
            self.state.borrow_mut().response.as_mut().unwrap().header(name, value);

            return true;
        }

        // Is this the end of the response header?
        if line == "\r\n" {
            self.state.borrow_mut().response_header_read = true;
            return true;
        }

        // Unknown header line we don't know how to parse.
        false
    }

    /// Called by curl when a chunk of the response body is read.
    fn write(&mut self, data: &[u8]) -> Result<usize, curl::easy::WriteError> {
        self.state.borrow_mut().buffer.extend(data);
        Ok(data.len())
    }
}
