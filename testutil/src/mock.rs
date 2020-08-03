//! A tiny mock HTTP server implementation to allow tests to inspect outgoing
//! requests and return specific responses.
//!
//! This is intentionally minimal and relatively low-level so that tests can
//! verify that Isahc is using the HTTP protocol properly.
//!
//! Only HTTP/1.x is implemented, as newer HTTP versions are mostly the same
//! semantically and are far more complex to deal with.

use crossbeam_channel::{Receiver, Sender};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::Cursor,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    thread,
};
use tiny_http::Server;

lazy_static::lazy_static! {
    static ref SHARED: Mutex<Option<Weak<Server>>> = Mutex::new(None);
    static ref MOCKS: Mutex<HashMap<String, MockChannel>> = Mutex::new(HashMap::new());
}

/// A mock HTTP endpoint.
pub struct Mock {
    prefix: String,
    server: Arc<Server>,
    request_rx: Receiver<MockRequest>,
    requests_received: RefCell<Vec<MockRequest>>,
    response_tx: Sender<MockResponse>,
}

impl Mock {
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        let prefix = format!("/{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
        let (request_tx, request_rx) = crossbeam_channel::unbounded();
        let (response_tx, response_rx) = crossbeam_channel::unbounded();

        MOCKS.lock().unwrap().insert(prefix.clone(), MockChannel {
            request_tx,
            response_rx,
        });

        Self {
            prefix,
            server: get_or_create_server(),
            request_rx,
            requests_received: RefCell::new(Vec::new()),
            response_tx,
        }
    }

    pub fn with_response(self, response: MockResponse) -> Self {
        self.response_tx.send(response).unwrap();
        self
    }

    pub fn url(&self) -> String {
        format!("http://{}{}", self.server.server_addr(), self.prefix)
    }

    /// Get the first request received by this mock.
    pub fn request(&self) -> MockRequest {
        self.drain_request_channel();
        self.requests_received
            .borrow()
            .get(0)
            .expect("no request received")
            .clone()
    }

    /// Get all requests received by this mock.
    pub fn requests(&self) -> Vec<MockRequest> {
        self.drain_request_channel();
        self.requests_received.borrow().clone()
    }

    fn drain_request_channel(&self) {
        for request in self.request_rx.try_iter() {
            self.requests_received.borrow_mut().push(request);
        }
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        MOCKS.lock().unwrap().remove(&self.prefix);
    }
}

#[derive(Clone)]
struct MockChannel {
    request_tx: Sender<MockRequest>,
    response_rx: Receiver<MockResponse>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockRequest {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl MockRequest {
    pub fn expect_header(&self, name: &str, value: &str) {
        self.headers.iter()
            .find(|(n, v)| n.to_lowercase() == name.to_lowercase() && v == value)
            .unwrap_or_else(|| panic!("no header named `{}` with value `{}` found", name, value));
    }
}

#[derive(Clone, Debug)]
pub struct MockResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

fn get_or_create_server() -> Arc<Server> {
    let mut shared = SHARED.lock().unwrap();

    if let Some(weak) = shared.take() {
        if let Some(server) = weak.upgrade() {
            return server;
        }
    }

    let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
    *shared = Some(Arc::downgrade(&server));

    {
        let server = server.clone();

        thread::spawn(move || {
            server_main(&server);
        });
    }

    server
}

fn server_main(server: &Server) {
    for request in server.incoming_requests() {
        let response = handle_request(&request);
        request.respond(response).unwrap();
    }
}

fn handle_request(request: &tiny_http::Request) -> tiny_http::Response<Cursor<Vec<u8>>> {
    let mock = MOCKS.lock().unwrap()
        .iter()
        .find(|(prefix, _)| request.url().starts_with(prefix.as_str()))
        .map(|(_, resource)| resource.clone());

    if let Some(mock) = mock {
        // Record the request that was received.
        mock.request_tx.send(MockRequest {
            method: request.method().to_string(),
            uri: request.url().to_owned(),
            headers: request
                .headers()
                .iter()
                .map(|header| (header.field.to_string(), header.value.to_string()))
                .collect(),
            body: None,
        }).ok();

        if let Ok(response) = mock.response_rx.try_recv() {
            let len = response.body.len();

            return tiny_http::Response::new(
                response.status_code.into(),
                vec![],
                Cursor::new(response.body),
                Some(len),
                None,
            );
        }
    }

    tiny_http::Response::new(404.into(), vec![], Cursor::new(Vec::new()), Some(0), None)
}
