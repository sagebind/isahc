use error::Error;
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom};


/// Contains the body of an HTTP request or response.
pub enum Body {
    /// An empty body.
    Empty,
    /// A body stored as a byte array.
    Bytes(Cursor<Vec<u8>>),
    /// A body read from a stream.
    Streaming(Box<Read + Send>),
}

impl Body {
    /// Create a body from a reader.
    pub fn from_reader<R: Read + Send + 'static>(reader: R) -> Body {
        Body::Streaming(Box::new(reader))
    }

    /// Report if this body is defined as empty.
    pub fn is_empty(&self) -> bool {
        match self {
            &Body::Empty => true,
            _ => false,
        }
    }

    /// Get the size of the body, if known.
    pub fn len(&self) -> Option<usize> {
        match self {
            &Body::Empty => Some(0),
            &Body::Bytes(ref bytes) => Some(bytes.get_ref().len()),
            &Body::Streaming(_) => None,
        }
    }

    /// Get the response body as a string.
    pub fn text(&mut self) -> Result<String, Error> {
        match self {
            &mut Body::Empty => Ok(String::new()),
            &mut Body::Bytes(ref bytes) => String::from_utf8(bytes.get_ref().clone()).map_err(Into::into),
            &mut Body::Streaming(ref mut reader) => {
                let mut string = String::new();
                reader.read_to_string(&mut string)?;
                Ok(string)
            },
        }
    }

    /// Attempt to parse the response as JSON.
    #[cfg(feature = "json")]
    pub fn json(&mut self) -> Result<::json::JsonValue, Error> {
        let text = self.text()?;
        Ok(::json::parse(&text)?)
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            &mut Body::Empty => Ok(0),
            &mut Body::Bytes(ref mut bytes) => bytes.read(buf),
            &mut Body::Streaming(ref mut reader) => reader.read(buf),
        }
    }
}

impl Seek for Body {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            &mut Body::Bytes(ref mut bytes) => bytes.seek(pos),
            _ => Err(io::ErrorKind::InvalidInput.into()),
        }
    }
}

impl Default for Body {
    fn default() -> Body {
        Body::Empty
    }
}

impl From<Vec<u8>> for Body {
    fn from(body: Vec<u8>) -> Body {
        Body::Bytes(Cursor::new(body))
    }
}

impl<'a> From<&'a [u8]> for Body {
    fn from(body: &'a [u8]) -> Body {
        body.to_owned().into()
    }
}

impl From<String> for Body {
    fn from(body: String) -> Body {
        body.into_bytes().into()
    }
}

impl<'a> From<&'a str> for Body {
    fn from(body: &'a str) -> Body {
        body.as_bytes().into()
    }
}

impl From<File> for Body {
    fn from(body: File) -> Body {
        Body::Streaming(Box::new(body))
    }
}
