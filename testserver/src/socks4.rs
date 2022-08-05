//! A simple SOCKS4 server implementation.

use std::{
    io::{self, BufRead, BufReader, Write},
    net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::Arc,
};

use crate::pool::pool;

#[derive(Clone, Debug)]
pub struct Socks4Server {
    listener: Arc<TcpListener>,
    addr: SocketAddr,
}

impl Socks4Server {
    /// Create a new SOCKS4 server listening at the given address.
    pub fn new(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;

        Ok(Self {
            addr: listener.local_addr()?,
            listener: Arc::new(listener),
        })
    }

    /// Get the address the server is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn run(&self) {
        for connection in self.listener.incoming().flatten() {
            let s = self.clone();

            pool().execute(move || {
                s.handle(connection).unwrap();
            });
        }
    }

    pub fn spawn(self) {
        pool().execute(move || self.run());
    }

    fn handle(&self, connection: TcpStream) -> io::Result<()> {
        let mut client_reader = BufReader::new(connection.try_clone()?);
        let mut client_writer = connection;

        // Read connect packet.
        client_reader.fill_buf()?;

        // Header
        assert_eq!(client_reader.buffer()[0], 4);
        assert_eq!(client_reader.buffer()[1], 1);
        client_reader.consume(2);

        // Destination port
        let upstream_port =
            u16::from_be_bytes([client_reader.buffer()[0], client_reader.buffer()[1]]);
        client_reader.consume(2);

        // Destination address
        let upstream_ip = IpAddr::from([
            client_reader.buffer()[0],
            client_reader.buffer()[1],
            client_reader.buffer()[2],
            client_reader.buffer()[3],
        ]);
        client_reader.consume(4);

        // User ID
        loop {
            let byte = client_reader.buffer()[0];
            client_reader.consume(1);
            if byte == 0 {
                break;
            }
        }

        // Connect to upstream.
        let mut upstream_writer = TcpStream::connect((upstream_ip, upstream_port))?;
        let mut upstream_reader = upstream_writer.try_clone()?;

        // Send response packet.
        client_writer.write_all(&[0, 0x5a, 0, 0, 0, 0, 0, 0])?;
        client_writer.flush()?;

        // Copy bytes in and to the upstream in parallel.
        pool().execute(move || {
            io::copy(&mut client_reader, &mut upstream_writer).unwrap();
        });

        io::copy(&mut upstream_reader, &mut client_writer)?;
        client_writer.shutdown(Shutdown::Both)?;

        Ok(())
    }
}
