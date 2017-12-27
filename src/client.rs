use std::io;
use std::io::Read;
use std::sync::{Arc, Mutex, Weak};
use transport::Transport;
use super::*;


/// An HTTP client for making requests.
///
/// The client maintains a connection pool internally and is expensive to create, so we recommend re-using your clients
/// instead of discarding and recreating them.
pub struct Client {
    max_connections: Option<u16>,
    options: Options,
    transport_pool: Arc<Mutex<Vec<Transport>>>,
    transport_count: u16,
}

impl Default for Client {
    fn default() -> Client {
        Client::new()
    }
}

impl Client {
    /// Create a new HTTP client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Create a new HTTP client using the default configuration.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Sends a GET request.
    pub fn get(&self, uri: &str) -> Result<Response, Error> {
        let request = http::Request::get(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a POST request.
    pub fn post<B: Into<Body>>(&self, uri: &str, body: B) -> Result<Response, Error> {
        let request = http::Request::post(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a PUT request.
    pub fn put<B: Into<Body>>(&self, uri: &str, body: B) -> Result<Response, Error> {
        let request = http::Request::put(uri).body(body.into())?;
        self.send(request)
    }

    /// Sends a DELETE request.
    pub fn delete(&self, uri: &str) -> Result<Response, Error> {
        let request = http::Request::delete(uri).body(Body::Empty)?;
        self.send(request)
    }

    /// Sends a request and returns the response.
    pub fn send(&self, request: Request) -> Result<Response, Error> {
        if let Some(mut transport) = self.get_transport() {
            let mut response = transport.execute(request)?;
            let stream = self.create_stream(transport);

            response
                .body(Body::from_reader(stream))
                .map_err(Into::into)
        } else {
            Err(Error::TooManyConnections)
        }
    }

    fn get_transport(&self) -> Option<Transport> {
        let mut pool = self.transport_pool.lock().unwrap();

        if let Some(transport) = pool.pop() {
            return Some(transport);
        }

        if let Some(max) = self.max_connections {
            if self.transport_count >= max {
                return None;
            }
        }

        Some(self.create_transport())
    }

    fn create_transport(&self) -> Transport {
        Transport::with_options(self.options.clone())
    }

    fn create_stream(&self, transport: Transport) -> Stream {
        Stream {
            pool: Arc::downgrade(&self.transport_pool),
            transport: Some(transport),
        }
    }
}


/// An HTTP client builder.
#[derive(Clone)]
pub struct ClientBuilder {
    max_connections: Option<u16>,
    options: Options,
}

impl Default for ClientBuilder {
    fn default() -> ClientBuilder {
        ClientBuilder {
            max_connections: Some(8),
            options: Default::default(),
        }
    }
}

impl ClientBuilder {
    /// Set the maximum number of connections the client should keep in its connection pool.
    ///
    /// To allow simultaneous requests, the client keeps a pool of multiple transports to pull from when performing a
    /// request. Reusing transports also improves performance if TCP keepalive is enabled. Increasing this value may
    /// improve performance when making many or frequent requests to the same server, but will also use more memory.
    ///
    /// Setting this to `0` will cause the client to not reuse any connections and the client will open a new connection
    /// for every request. Setting this to `None` will allow unlimited simultaneous connections.
    ///
    /// The default value is `Some(8)`.
    pub fn max_connections(mut self, max: Option<u16>) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the connection options to use.
    pub fn options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    /// Build an HTTP client using the configured options.
    pub fn build(self) -> Client {
        Client {
            max_connections: self.max_connections,
            options: self.options,
            transport_pool: Arc::new(Mutex::new(Vec::new())),
            transport_count: 0,
        }
    }
}


/// Stream that reads the response body incrementally.
///
/// A stream object will hold on to the connection that initiated the request until the entire response is read or the
/// stream is dropped.
struct Stream {
    pool: Weak<Mutex<Vec<Transport>>>,
    transport: Option<Transport>,
}

impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.transport.as_mut().unwrap().read(buffer)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Some(transport) = self.transport.take() {
            if let Some(pool) = self.pool.upgrade() {
                pool.lock()
                    .unwrap()
                    .push(transport);
            }
        }
    }
}
