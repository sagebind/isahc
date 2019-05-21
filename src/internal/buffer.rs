//! Custom byte buffer structures designed for chttp's use cases.
//!
//! Generally a ring buffer is an efficient and appropriate data structure for
//! asynchronously transmitting a stream of bytes between two threads that also
//! gives you control over memory allocation to avoid consuming an unknown
//! amount of memory. Setting a fixed memory limit also gives you a degree of
//! flow control if the producer ends up being faster than the consumer.
//!
//! But for chttp a ring buffer will not work because of how curl's write
//! callbacks are designed. Curl has its own internal buffer management, which
//! we borrow a slice of when receiving data. The size of this slice is unknown,
//! and we must consume all of it at once or none of it.
//!
//! Because of these constraints, instead we use a quite unique type of buffer
//! that uses a fixed number of growable buffers that are exchanged back and
//! forth between a producer and a consumer. Since each buffer is a vector, it
//! can grow to whatever size is required of it in order to fit a single curl
//! chunk.
//!
//! To avoid the constant allocation overhead of creating a new buffer for every
//! chunk, after a consumer finishes reading from a buffer, it returns the
//! buffer to the producer over a channel to be reused. The number of buffers
//! available in this system is fixed at creation time, so the only allocations
//! that happen during reads and writes are occasional reallocation for each
//! individual vector to fit larger chunks of bytes that don't already fit.

use std::io::{self, Cursor, Read, Write};

pub fn bounded(size: usize) -> (Reader, Writer) {
    let (buf_pool_tx, buf_pool_rx) = crossbeam_channel::bounded(size);
    let (buf_stream_tx, buf_stream_rx) = crossbeam_channel::bounded(size);

    // Fill up the buffer pool.
    for _ in 0..size {
        buf_pool_tx.send(Vec::new()).expect("buffer pool overflow");
    }

    debug_assert!(buf_pool_tx.is_full());

    let reader = Reader {
        buf_pool_tx,
        buf_stream_rx,
        current: None,
    };

    let writer = Writer {
        buf_pool_rx,
        buf_stream_tx,
    };

    (reader, writer)
}

pub struct Reader {
    buf_pool_tx: crossbeam_channel::Sender<Vec<u8>>,
    buf_stream_rx: crossbeam_channel::Receiver<Vec<u8>>,
    current: Option<Cursor<Vec<u8>>>,
}

impl Read for Reader {
    fn read(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        // Fetch the buffer to read from. If we already have one from a previous
        // read, use that, otherwise receive the next buffer from the writer.
        let mut buffer = match self.current.take() {
            Some(buffer) => buffer,

            None => match self.buf_stream_rx.try_recv() {
                Ok(buf) => Cursor::new(buf),
                Err(crossbeam_channel::TryRecvError::Empty) => return Err(io::ErrorKind::WouldBlock.into()),
                Err(crossbeam_channel::TryRecvError::Disconnected) => return Err(io::ErrorKind::BrokenPipe.into()),
            }
        };

        // Do the read.
        let len = buffer.read(dest)?;

        // If the buffer is not empty yet, keep it for a future read.
        if buffer.position() < buffer.get_ref().len() as u64 {
            self.current = Some(buffer);
        }

        // Otherwise, return it to the writer to be reused.
        else {
            let mut buffer = buffer.into_inner();
            buffer.clear();

            match self.buf_pool_tx.try_send(buffer) {
                Ok(()) => {},
                // We pre-fill the buffer pool channel with an exact number of
                // buffers, so this can never happen.
                Err(crossbeam_channel::TrySendError::Full(_)) => panic!("buffer pool overflow"),
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => return Err(io::ErrorKind::BrokenPipe.into()),
            }
        }

        Ok(len)
    }
}

/// Writing half of a buffer.
///
/// Writing to this buffer will never block. If the buffer is full, a
/// `WouldBlock` error will be returned. If the reader disconnects, a
/// `BrokenPipe` will be returned.
pub struct Writer {
    buf_pool_rx: crossbeam_channel::Receiver<Vec<u8>>,
    buf_stream_tx: crossbeam_channel::Sender<Vec<u8>>,
}

impl Write for Writer {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        // Ensure zero-length buffers don't get into the stream because it makes
        // the code more complicated.
        if src.is_empty() {
            return Ok(0);
        }

        match self.buf_pool_rx.try_recv() {
            Ok(mut buf) => {
                buf.extend_from_slice(src);
                self.buf_stream_tx.send(buf).unwrap();
                Ok(src.len())
            },
            Err(crossbeam_channel::TryRecvError::Empty) => Err(io::ErrorKind::WouldBlock.into()),
            Err(crossbeam_channel::TryRecvError::Disconnected) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_then_write() {
        let (mut reader, mut writer) = bounded(1);

        assert_eq!(writer.write(b"hello").unwrap(), 5);
        assert_eq!(writer.write(b"world").unwrap_err().kind(), io::ErrorKind::WouldBlock);

        let mut dest = [0; 5];
        assert_eq!(reader.read(&mut dest).unwrap(), 5);
        assert_eq!(&dest, b"hello");
        assert_eq!(reader.read(&mut dest).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }
}
