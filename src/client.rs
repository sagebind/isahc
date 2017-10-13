use curl;
use http;
use std::io::{self, Read};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use super::{Entity, Request, Response};
use transfer::{Collector, Transfer};


pub struct Client {
    /// Pool of cURL easy handles.
    pool: Arc<Pool>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            pool: Arc::new(Pool::new()),
        }
    }

    pub fn get(&self, uri: &str) -> Response {
        let request = http::Request::get(uri).body(::Entity::Empty).unwrap();
        self.send(request)
    }

    pub fn send(&self, request: Request) -> Response {
        // Prepare a new transfer.
        let transfer = Transfer::new(request);

        // Get a handle.
        let handle = self.pool.take()
            .unwrap_or_else(|| Connection::new(Some(Arc::downgrade(&self.pool))));

        // Execute the transfer.
        handle.execute(transfer)
    }
}


pub struct Pool {
    connections: Mutex<Vec<Connection>>,
}

impl Pool {
    pub fn new() -> Pool {
        Pool {
            connections: Mutex::new(Vec::new()),
        }
    }

    pub fn take(&self) -> Option<Connection> {
        self.connections.lock().unwrap().pop()
    }

    pub fn submit(&self, connection: Connection) {
        self.connections.lock()
            .unwrap()
            .push(connection);
    }
}


pub struct Connection {
    /// Connection pool this connection belongs to.
    pool: Option<Weak<Pool>>,

    /// A curl multi handle used to execute transfers. Also holds the internal connection pool.
    multi: curl::multi::Multi,

    /// The easy handle for the current transfer being executed, if any.
    easy: Option<curl::multi::Easy2Handle<Collector>>,
}

impl Connection {
    /// Create a new connection.
    pub fn new(pool: Option<Weak<Pool>>) -> Connection {
        Connection {
            pool: pool,
            multi: curl::multi::Multi::new(),
            easy: None,
        }
    }

    /// Execute a transfer.
    pub fn execute(mut self, transfer: Transfer) -> Response {
        let easy_handle = self.multi.add2(transfer.create_handle()).unwrap();
        self.easy = Some(easy_handle);

        // Wait for the headers to be read.
        while !transfer.is_response_header_read() {
            self.multi.perform().unwrap();
        }

        let mut response_builder = transfer.take_response_builder();

        // All that is left is the body, so create a stream.
        let stream = Stream {
            connection: Some(self),
            transfer: transfer,
        };

        response_builder.body(Entity::Stream(Box::new(stream))).unwrap()
    }

    /// Close the current transfer.
    pub fn close(&mut self) {
        if let Some(easy) = self.easy.take() {
            self.multi.remove2(easy).unwrap();
        }
    }

    pub fn return_to_pool(mut self) {
        if let Some(pool) = self.pool.take() {
            // Check if the handle pool still exists.
            if let Some(pool) = pool.upgrade() {
                // Return the handle back to where it came from to be reused.
                pool.submit(self);
            }
        }
    }
}


/// Stream that reads the response body incrementally.
///
/// A stream object will hold on to the connection that initiated the request until the entire response is read or the
/// stream is dropped.
pub struct Stream {
    connection: Option<Connection>,

    /// The current transfer being streamed.
    transfer: Transfer,
}

impl Stream {
    /// Return the connection back to the connection pool to be reused.
    fn return_connection(&mut self) {
        if let Some(connection) = self.connection.take() {
            connection.return_to_pool();
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut pos = 0;

        while pos < buffer.len() {
            if let Some(byte) = self.transfer.buffer_read() {
                buffer[pos] = byte;
                pos += 1;
                continue;
            }

            if self.transfer.is_complete() {
                break;
            }

            self.connection.as_ref().unwrap().multi.wait(&mut [], Duration::from_secs(1)).unwrap();

            match self.connection.as_ref().unwrap().multi.perform() {
                // No more transfers are active.
                Ok(0) => {
                    self.transfer.complete();
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

impl Drop for Stream {
    fn drop(&mut self) {
        self.return_connection();
    }
}
