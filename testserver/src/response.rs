use std::io::Cursor;

#[derive(Clone, Debug)]
pub struct MockResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl MockResponse {
    pub(crate) fn into_http_response(self) -> tiny_http::Response<Cursor<Vec<u8>>> {
        let len = self.body.len();

        tiny_http::Response::new(
            self.status_code.into(),
            vec![],
            Cursor::new(self.body),
            Some(len),
            None,
        )
    }
}

impl Default for MockResponse {
    fn default() -> Self {
        Self {
            status_code: 200,
            headers: Vec::new(),
            body: b"OK".to_vec(),
        }
    }
}
