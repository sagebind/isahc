use crate::{request::Request, response::Response, server::Server};
use regex::Regex;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    io::{self, Cursor},
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

lazy_static::lazy_static! {
    static ref PREFIX_REGEX: Regex = Regex::new(r"^/(\d+)($|[/?#])").unwrap();
    static ref REGISTRY: Mutex<HashMap<usize, Endpoint>> = Mutex::new(HashMap::new());
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct EndpointBuilder {
    static_response: Response,
}

impl EndpointBuilder {
    pub fn status_code(mut self, status_code: u16) -> Self {
        self.static_response.status_code = status_code;
        self
    }

    pub fn header(mut self, name: impl fmt::Display, value: impl fmt::Display) -> Self {
        self.static_response.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        let body = body.into();
        self.static_response.body_len = Some(body.len());
        self.static_response.body = Arc::new(move || body.clone());
        self
    }

    pub fn body_fn(mut self, f: impl Fn(&mut dyn io::Write) -> io::Result<()> + Send + Sync + 'static) -> Self {
        self.static_response.body_len = None;
        self.static_response.body = Arc::new(move || {
            let mut buf = Cursor::new(Vec::new());
            f(&mut buf).unwrap();
            buf.into_inner()
        });
        self
    }

    pub fn build(self) -> Endpoint {
        let endpoint = Endpoint(Arc::new(Inner {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            server: Server::shared(),
            requests: Mutex::new(VecDeque::new()),
            response_fn: Box::new(move |_| self.static_response.clone()),
        }));

        REGISTRY.lock().unwrap().insert(endpoint.0.id, endpoint.clone());

        endpoint
    }
}

/// A mock HTTP endpoint.
#[derive(Clone)]
pub struct Endpoint(Arc<Inner>);

struct Inner {
    id: usize,
    server: Arc<Server>,
    requests: Mutex<VecDeque<Request>>,
    response_fn: Box<dyn Fn(&Request) -> Response + Send + Sync>,
}

impl Endpoint {
    pub fn builder() -> EndpointBuilder {
        EndpointBuilder {
            static_response: Response::default(),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}/{}", self.addr(), self.0.id)
    }

    pub fn addr(&self) -> SocketAddr {
        self.0.server.addr()
    }

    /// Get the first request received by this mock.
    pub fn request(&self) -> Request {
        self.0
            .requests
            .lock()
            .unwrap()
            .get(0)
            .expect("no request received")
            .clone()
    }

    /// Get all requests received by this mock.
    pub fn requests(&self) -> Vec<Request> {
        self.0.requests.lock().unwrap().iter().cloned().collect()
    }

    pub(crate) fn find(url: &str) -> Option<Self> {
        if let Some(captures) = PREFIX_REGEX.captures(url) {
            let id = captures.get(1).unwrap().as_str().parse().unwrap();

            REGISTRY
                .lock()
                .unwrap()
                .get(&id)
                .map(|endpoint| endpoint.clone())
        } else {
            None
        }
    }

    pub(crate) fn handle(&self, request: Request) -> Response {
        let response = (&*self.0.response_fn)(&request);
        self.0.requests.lock().unwrap().push_back(request);
        response
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        REGISTRY.lock().unwrap().remove(&self.id);
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoint")
            .field("id", &self.0.id)
            .finish()
    }
}
