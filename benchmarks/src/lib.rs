use rouille::{Request, Response};
use std::{net::SocketAddr, sync::Arc, thread};

pub struct TestServer {
    addr: SocketAddr,
    counter: Option<Arc<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    pub fn static_response(body: &'static [u8]) -> Self {
        Self::new(move |_| {
            use std::io::Cursor;

            rouille::Response {
                status_code: 200,
                headers: vec![],
                data: rouille::ResponseBody::from_reader(Cursor::new(body)),
                upgrade: None,
            }
        })
    }

    pub fn new(handler: impl Send + Sync + 'static + Fn(&Request) -> Response) -> Self {
        let server = rouille::Server::new("localhost:0", handler).unwrap();
        let addr = server.server_addr();

        let counter_outer = Arc::new(());
        let counter_inner = counter_outer.clone();
        let handle = thread::spawn(move || {
            while Arc::strong_count(&counter_inner) > 1 {
                server.poll();
            }
        });

        Self {
            addr,
            counter: Some(counter_outer),
            handle: Some(handle),
        }
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.counter.take();
        self.handle.take().unwrap().join().unwrap();
    }
}
