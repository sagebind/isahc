//! A tiny mock HTTP server implementation to allow tests to inspect outgoing
//! requests and return specific responses.
//!
//! This is intentionally minimal and relatively low-level so that tests can
//! verify that Isahc is using the HTTP protocol properly.
//!
//! Only HTTP/1.x is implemented, as newer HTTP versions are mostly the same
//! semantically and are far more complex to deal with.

use crate::{
    request::MockRequest,
    responder::*,
    response::MockResponse,
};
use std::{
    collections::VecDeque,
    io::{Cursor, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tiny_http::Server;

/// A mock HTTP endpoint.
pub struct Mock<R> {
    server: Arc<Server>,
    requests: Arc<Mutex<VecDeque<MockRequest>>>,
    responder: Arc<R>,
}

impl<R: Responder> Mock<R> {
    pub fn new(responder: R) -> Self {
        let mock = Self {
            server: Arc::new(Server::http("127.0.0.1:0").unwrap()),
            requests: Default::default(),
            responder: Arc::new(responder),
        };

        thread::spawn({
            let mock = mock.clone();

            move || {
                for request in mock.server.incoming_requests() {
                    mock.handle_request(request);
                }
            }
        });

        mock.wait_until_ready();

        mock
    }

    pub fn addr(&self) -> SocketAddr {
        self.server.server_addr()
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr())
    }

    /// Get the first request received by this mock.
    pub fn request(&self) -> MockRequest {
        self.requests.lock()
            .unwrap()
            .get(0)
            .expect("no request received")
            .clone()
    }

    /// Get all requests received by this mock.
    pub fn requests(&self) -> Vec<MockRequest> {
        self.requests.lock().unwrap().iter().cloned().collect()
    }

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

    fn respond(&self, request: MockRequest) -> MockResponse {
        if let Some(response) = self.responder.respond(request.clone()) {
            return response;
        }

        MockResponse {
            status_code: 404,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn handle_request(&self, mut request: tiny_http::Request) {
        if request.headers().iter().find(|h| h.field.as_str() == "host" && h.value == "api.mock.local").is_some() {
            if let Some(response) = self.handle_api_request(&request) {
                request.respond(response).unwrap();
                return;
            }
        }

        let mut body = Vec::new();

        if let Some(len) = request.body_length() {
            body.reserve(len);
        }

        request.as_reader().read_to_end(&mut body).unwrap();

        // Build a record of the request received.
        let mock_request = MockRequest {
            method: request.method().to_string(),
            url: request.url().to_string(),
            headers: request.headers()
                .iter()
                .map(|header| (header.field.to_string(), header.value.to_string()))
                .collect(),
            body: Some(body),
        };

        let response = self.respond(mock_request.clone());

        request.respond(response.into_http_response()).unwrap();

        self.requests.lock().unwrap().push_back(mock_request);
    }

    fn handle_api_request(&self, request: &tiny_http::Request) -> Option<tiny_http::Response<Cursor<Vec<u8>>>> {
        if request.url() == "/health" {
            Some(tiny_http::Response::new(
                200.into(),
                vec![],
                Cursor::new((&b"OK"[..]).into()),
                Some(2),
                None,
            ))
        } else {
            None
        }
    }
}

impl Default for Mock<DefaultResponder> {
    fn default() -> Self {
        Self::new(DefaultResponder)
    }
}

impl<R> Clone for Mock<R> {
    fn clone(&self) -> Self {
        Self {
            server: self.server.clone(),
            requests: self.requests.clone(),
            responder: self.responder.clone(),
        }
    }
}
