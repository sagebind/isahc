//! A tiny mock HTTP server implementation to allow tests to inspect outgoing
//! requests and return specific responses.
//!
//! This is intentionally minimal and relatively low-level so that tests can
//! verify that Isahc is using the HTTP protocol properly.
//!
//! Only HTTP/1.x is implemented, as newer HTTP versions are mostly the same
//! semantically and are far more complex to deal with.

use crate::{pool::pool, request::Request, responder::*, response::Response};
use std::{
    collections::VecDeque,
    io::{Cursor, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
        Mutex,
    },
    thread,
    time::Duration,
};
use tiny_http::Server;

/// A mock HTTP endpoint.
#[derive(Clone)]
pub struct Mock(Arc<Inner>);

struct Inner {
    server: Server,

    requests: Mutex<VecDeque<Request>>,

    /// Number of requests received since the mock was created.
    request_counter: AtomicU32,

    /// A list of responders. When receiving a request each responder is tried
    /// in order until one returns a response.
    responders: Vec<Box<dyn Responder>>,
}

impl Mock {
    /// Create a new mock server with a single responder.
    pub fn new<R: Responder>(responder: R) -> Self {
        Self::builder().responder(responder).build()
    }

    /// Create a builder for creating a customized mock server.
    pub fn builder() -> Builder {
        Builder {
            responders: vec![],
        }
    }

    /// Get the socket address of this mock server.
    pub fn addr(&self) -> SocketAddr {
        self.0.server.server_addr()
    }

    /// Get the HTTP URL of this mock server.
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr())
    }

    /// Get the number of requests received so far by this mock.
    pub fn requests_received(&self) -> u32 {
        self.0.request_counter.load(Ordering::SeqCst)
    }

    /// Get the first request received by this mock.
    pub fn request(&self) -> Request {
        let request = self.0.requests.lock().unwrap().front().cloned();

        request.expect("no request received")
    }

    #[rustfmt::skip]
    fn is_ready(&self) -> bool {
        TcpStream::connect(self.addr())
            .and_then(|mut stream| {
                stream.write_all(b"\
                    GET /health HTTP/1.1\r\n\
                    host: api.mock.local\r\n\
                    connection: close\r\n\
                    \r\n\
                ")?;

                let mut response = Vec::new();
                stream.read_to_end(&mut response)?;

                Ok(response.ends_with(b"\r\nOK"))
            })
            .unwrap_or(false)
    }

    fn wait_until_ready(&self) {
        for _ in 0..9 {
            if self.is_ready() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }

        panic!("mock server did not become ready after 9 tries");
    }

    fn handle_request(&self, mut request: tiny_http::Request) {
        if request
            .headers()
            .iter()
            .any(|h| h.field.as_str() == "host" && h.value == "api.mock.local")
        {
            self.handle_api_request(request);
            return;
        }

        let mut body = Vec::new();

        if let Some(len) = request.body_length() {
            body.reserve(len);
        }

        request.as_reader().read_to_end(&mut body).unwrap();

        // Build a record of the request received.
        let mut mock_request = Request {
            number: self.0.request_counter.fetch_add(1, Ordering::SeqCst),
            method: request.method().to_string(),
            url: request.url().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|header| (header.field.to_string(), header.value.to_string()))
                .collect(),
            body: Some(body),
        };

        self.0
            .requests
            .lock()
            .unwrap()
            .push_back(mock_request.clone());

        let mut ctx = RequestContext::new(&mut mock_request, request);

        for responder in &self.0.responders {
            responder.respond(&mut ctx);

            if ctx.http_request.is_none() {
                break;
            }
        }

        if let Some(request) = ctx.http_request.take() {
            request
                .respond(
                    Response {
                        status_code: 404,
                        headers: Vec::new(),
                        body: Box::new(std::io::empty()),
                        body_len: Some(0),
                    }
                    .into_http_response(),
                )
                .unwrap();
        }
    }

    fn handle_api_request(&self, request: tiny_http::Request) {
        if request.url() == "/health" {
            let _ = request.respond(tiny_http::Response::new(
                200.into(),
                vec![],
                Cursor::new(b"OK".to_vec()),
                Some(2),
                None,
            ));
        }
    }
}

/// A builder for creating mock servers.
pub struct Builder {
    responders: Vec<Box<dyn Responder>>,
}

impl Builder {
    /// Add a responder to the mock. Responders are tried in the order that they
    /// are added to the builder.
    pub fn responder<R: Responder + 'static>(mut self, responder: R) -> Self {
        self.responders.push(Box::new(responder));
        self
    }

    /// Start a new mock server.
    pub fn build(self) -> Mock {
        let mock = Mock(Arc::new(Inner {
            server: Server::http("127.0.0.1:0").unwrap(),
            requests: Default::default(),
            request_counter: AtomicU32::new(0),
            responders: self.responders,
        }));

        pool().execute({
            let mock = mock.clone();

            move || {
                for request in mock.0.server.incoming_requests() {
                    mock.handle_request(request);
                }
            }
        });

        mock.wait_until_ready();

        mock
    }
}
