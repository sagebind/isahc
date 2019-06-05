use env_logger;
use rouille;
use std::env;
use std::net::SocketAddr;
use std::thread;
use std::sync::Arc;
use std::sync::Once;

pub fn spawn(handler: fn(&rouille::Request) -> rouille::Response) -> Server {
    let server = rouille::Server::new("localhost:0", handler).unwrap();
    let addr = server.server_addr();

    let counter_outer = Arc::new(());
    let counter_inner = counter_outer.clone();
    let handle = thread::spawn(move || {
        while Arc::strong_count(&counter_inner) > 1 {
            server.poll();
        }
    });

    Server {
        addr: addr,
        counter: Some(counter_outer),
        handle: Some(handle),
    }
}

pub struct Server {
    addr: SocketAddr,
    counter: Option<Arc<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Server {
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.counter.take();
        self.handle.take().unwrap().join().unwrap();
    }
}
