use std::{io::{BufRead, BufReader, Read, Result}, net::TcpListener};

use httparse::parse_headers;

pub(crate) struct Server {
    listener: TcpListener,
}

impl Server {
    pub(crate) fn accept(&mut self) -> Result<Connection> {
        let (stream, addr) = self.listener.accept()?;

        let mut reader = GrowableBufReader::new(stream);
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut request = httparse::Request::new(&mut headers);

        loop {
            reader.fill_buf_additional(8192)?;

            let result = request.parse(reader.buffer());
            match result {
                Ok(httparse::Status::Partial) => continue,
                Ok(httparse::Status::Complete(offset)) => {
                    reader.consume(offset);
                },
                Err(_) => unimplemented!(),
            }
        }

        unimplemented!()
    }
}

pub(crate) struct Connection {

}

struct GrowableBufReader<R: Read> {
    inner: R,
    buffer: Vec<u8>,
    low: usize,
    high: usize,
}

impl<R: Read> GrowableBufReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(8192),
            low: 0,
            high: 0,
        }
    }

    #[inline]
    fn available(&self) -> usize {
        self.high - self.low
    }

    #[inline]
    fn buffer(&self) -> &[u8] {
        &self.buffer[self.low..self.high]
    }

    fn fill_buf_additional(&mut self, max: usize) -> Result<usize> {
        self.reserve(max);
        let amt = self.inner.read(&mut self.buffer[self.high..])?;
        self.high += amt;

        Ok(amt)
    }

    fn reserve(&mut self, capacity: usize) {
        let desired_buffer_size = self.high + capacity;

        if self.buffer.len() < desired_buffer_size {
            self.buffer.resize(desired_buffer_size, 0);
        }
    }
}

impl<R: Read> BufRead for GrowableBufReader<R> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.available() == 0 {
            self.fill_buf_additional(8192)?;
        }

        Ok(self.buffer())
    }

    fn consume(&mut self, amt: usize) {
        if amt >= self.available() {
            self.low = 0;
            self.high = 0;
            self.buffer.clear();
        } else {
            self.low += amt;
        }
    }
}

impl<R: Read> Read for GrowableBufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let src = self.fill_buf()?;
        let amt = buf.len().min(src.len());
        buf[..amt].copy_from_slice(&src[..amt]);
        self.consume(amt);

        Ok(amt)
    }
}
