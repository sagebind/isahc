use std::io;
use std::io::Read;
use std::sync::{Arc, Mutex, Weak};
use transport::Transport;
use super::*;


pub struct Client {
    pool: Arc<Pool>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            pool: Arc::new(Pool::new()),
        }
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

    pub fn send(&self, request: Request) -> Result<Response, Error> {
        let mut transport = self.pool.take()
            .unwrap_or_else(|| Transport::new());

        transport.begin(request).unwrap();
        let mut response_builder = transport.read_response().unwrap();

        let stream = Stream {
            pool: Arc::downgrade(&self.pool),
            transport: Some(transport),
        };

        response_builder
            .body(Body::from_reader(stream))
            .map_err(Into::into)
    }
}


pub struct Pool {
    transports: Mutex<Vec<Transport>>,
}

impl Pool {
    pub fn new() -> Pool {
        Pool {
            transports: Mutex::new(Vec::new()),
        }
    }

    pub fn take(&self) -> Option<Transport> {
        self.transports.lock().unwrap().pop()
    }

    pub fn submit(&self, transport: Transport) {
        self.transports.lock()
            .unwrap()
            .push(transport);
    }
}


/// Stream that reads the response body incrementally.
///
/// A stream object will hold on to the connection that initiated the request until the entire response is read or the
/// stream is dropped.
pub struct Stream {
    pool: Weak<Pool>,
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
                pool.submit(transport);
            }
        }
    }
}
