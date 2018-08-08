use bytes::Bytes;
use std::io::*;
use std::sync::mpsc;

pub struct Stream {
    reader: StreamReader,
    writer: StreamWriter,
}

impl Stream {
    pub fn new() -> Self {
        let channel = mpsc::channel();

        Self {
            reader: StreamReader {
                buffer: None,
                receiver: channel.1,
            },
            writer: StreamWriter {
                sender: channel.0,
            },
        }
    }

    pub fn split(self) -> (StreamReader, StreamWriter) {
        (self.reader, self.writer)
    }
}

pub struct StreamReader {
    buffer: Option<Bytes>,
    receiver: mpsc::Receiver<Bytes>,
}

impl Read for StreamReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let bytes = self.buffer.take().or_else(|| {
            self.receiver.recv().ok()
        });

        if let Some(mut bytes) = bytes {
            let length = buf.len().min(bytes.len());
            let remainder = bytes.split_off(length);

            &buf[..bytes.len()].copy_from_slice(&bytes);

            if !remainder.is_empty() {
                self.buffer = Some(remainder);
            }

            Ok(length)
        } else {
            Ok(0)
        }
    }
}

pub struct StreamWriter {
    sender: mpsc::Sender<Bytes>,
}

impl Write for StreamWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self.sender.send(Bytes::from(buf)) {
            Ok(()) => Ok(buf.len()),
            Err(_) => Err(ErrorKind::BrokenPipe.into()),
        }
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
