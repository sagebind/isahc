use std::io::{Cursor, Read};

pub struct Response {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read>,
    pub body_len: Option<usize>,
}

impl Response {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_body_buf(mut self, buf: impl Into<Vec<u8>>) -> Self {
        let buf = buf.into();
        self.body_len = Some(buf.len());
        self.body = Box::new(Cursor::new(buf));
        self
    }

    pub fn with_body_reader(mut self, reader: impl Read + 'static) -> Self {
        self.body_len = None;
        self.body = Box::new(reader);
        self
    }

    pub(crate) fn into_http_response(self) -> tiny_http::Response<Box<dyn Read>> {
        tiny_http::Response::new(
            self.status_code.into(),
            self.headers.into_iter()
                .map(|(name, value)| tiny_http::Header::from_bytes(
                    name.as_bytes(),
                    value.as_bytes(),
                ).unwrap())
                .collect(),
            self.body,
            self.body_len,
            None,
        )
    }
}

impl Default for Response {
    fn default() -> Self {
        Self {
            status_code: 200,
            headers: Vec::new(),
            body: Box::new(std::io::empty()),
            body_len: Some(0),
        }
    }
}
