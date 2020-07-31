//! A tiny mock HTTP server implementation to allow tests to inspect outgoing
//! requests and return specific responses.
//!
//! This is intentionally minimal and relatively low-level so that tests can
//! verify that Isahc is using the HTTP protocol properly.
//!
//! Only HTTP/1.x is implemented, as newer HTTP versions are mostly the same
//! semantically and are far more complex to deal with.

use crossbeam_channel::Receiver;
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::Cursor,
    net::SocketAddr,
    sync::Arc,
    thread,
};
use tiny_http::Server;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct MockResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub struct Mock {
    server: Arc<Server>,
    requests_rx: Receiver<Request>,
    requests_received: RefCell<Vec<Request>>,
}

impl Mock {
    pub fn builder() -> MockBuilder {
        MockBuilder {
            responses: VecDeque::new(),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.server.server_addr()
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr())
    }

    /// Get the first request received by this mock.
    pub fn request(&self) -> Request {
        self.drain_request_channel();
        self.requests_received.borrow()
            .get(0)
            .expect("no request received")
            .clone()
    }

    /// Get all requests received by this mock.
    pub fn requests(&self) -> Vec<Request> {
        self.drain_request_channel();
        self.requests_received.borrow().clone()
    }

    fn drain_request_channel(&self) {
        for request in self.requests_rx.try_iter() {
            self.requests_received.borrow_mut().push(request);
        }
    }
}

pub struct MockBuilder {
    responses: VecDeque<MockResponse>,
}

impl MockBuilder {
    pub fn with_response(mut self, response: MockResponse) -> Self {
        self.responses.push_back(response);
        self
    }

    pub fn build(mut self) -> Mock {
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let (requests_tx, requests_rx) = crossbeam_channel::unbounded();

        {
            let server = server.clone();

            thread::spawn(move || {
                for request in server.incoming_requests() {
                    // Build a record of the request received.
                    let mock_request = Request {
                        method: request.method().to_string(),
                        headers: request.headers()
                            .iter()
                            .map(|header| (header.field.to_string(), header.value.to_string()))
                            .collect(),
                        body: None,
                    };

                    let server_response = if let Some(response) = self.responses.pop_front() {
                        let len = response.body.len();

                        tiny_http::Response::new(
                            response.status_code.into(),
                            vec![],
                            Cursor::new(response.body),
                            Some(len),
                            None,
                        )
                    } else {
                        tiny_http::Response::new(
                            404.into(),
                            vec![],
                            Cursor::new(Vec::new()),
                            Some(0),
                            None,
                        )
                    };

                    request.respond(server_response).unwrap();
                    requests_tx.send(mock_request).ok();
                }
            });
        }

        Mock {
            server,
            requests_rx,
            requests_received: RefCell::new(Vec::new()),
        }
    }
}
