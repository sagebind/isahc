use std::io::Cursor;

#[derive(Clone, Debug)]
pub struct Response {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub transfer_encoding: bool,
}

impl Response {
    pub(crate) fn into_http_response(self) -> tiny_http::Response<Cursor<Vec<u8>>> {
        let len = self.body.len();

        tiny_http::Response::new(
            self.status_code.into(),
            self.headers.into_iter()
                .map(|(name, value)| tiny_http::Header::from_bytes(
                    name.as_bytes(),
                    value.as_bytes(),
                ).unwrap())
                .collect(),
            Cursor::new(self.body),
            if self.transfer_encoding {
                None
            } else {
                Some(len)
            },
            None,
        )
    }
}

impl Default for Response {
    fn default() -> Self {
        Self {
            status_code: 200,
            headers: Vec::new(),
            body: Vec::new(),
            transfer_encoding: false,
        }
    }
}
