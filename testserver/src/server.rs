use crate::{endpoint::Endpoint, request::Request};
use std::{
    io::Cursor,
    net::SocketAddr,
    sync::{Arc, Barrier, Mutex, Weak},
    thread,
};

lazy_static::lazy_static! {
    static ref SHARED: Mutex<Option<Weak<Server>>> = Mutex::new(None);
}

pub(crate) struct Server {
    server: Arc<tiny_http::Server>,
}

impl Server {
    pub(crate) fn shared() -> Arc<Self> {
        let mut shared = SHARED.lock().unwrap();

        if let Some(weak) = shared.as_ref() {
            if let Some(server) = weak.upgrade() {
                return server;
            }
        }

        let server = Arc::new(Self::new());
        *shared = Some(Arc::downgrade(&server));

        server
    }

    pub(crate) fn new() -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let barrier = Arc::new(Barrier::new(2));

        {
            let server = server.clone();
            let barrier = barrier.clone();

            thread::spawn(move || {
                barrier.wait();

                for mut request in server.incoming_requests() {
                    let response = handle(&mut request);
                    request.respond(response).ok();
                }
            });
        }

        barrier.wait();

        Self { server }
    }

    pub(crate) fn addr(&self) -> SocketAddr {
        self.server.server_addr()
    }
}

fn handle(request: &mut tiny_http::Request) -> tiny_http::Response<Cursor<Vec<u8>>> {
    if let Some(endpoint) = Endpoint::find(request.url()) {
        let mut body = Vec::new();

        if let Some(len) = request.body_length() {
            body.reserve(len);
        }

        request.as_reader().read_to_end(&mut body).unwrap();

        return endpoint.handle(Request {
            method: request.method().to_string(),
            uri: request.url().to_owned(),
            headers: request
                .headers()
                .iter()
                .map(|header| (header.field.to_string(), header.value.to_string()))
                .collect(),
            body: Some(body),
        }).into();
    }

    tiny_http::Response::new(404.into(), vec![], Cursor::new(Vec::new()), Some(0), None)
}
