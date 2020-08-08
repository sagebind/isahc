use std::{
    io::Cursor,
    sync::Arc,
};

#[derive(Clone)]
pub struct Response {
    pub(crate) status_code: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Arc<dyn Fn() -> Vec<u8> + Send + Sync + 'static>,
    pub(crate) body_len: Option<usize>,
}

impl Default for Response {
    fn default() -> Self {
        Self {
            status_code: 200,
            headers: Vec::new(),
            body: Arc::new(Vec::new),
            body_len: Some(0),
        }
    }
}

impl From<Response> for tiny_http::Response<Cursor<Vec<u8>>> {
    fn from(response: Response) -> Self {
        tiny_http::Response::new(
            response.status_code.into(),
            response.headers.into_iter()
                .filter_map(|(name, value)| tiny_http::Header::from_bytes(name, value).ok())
                .collect(),
            Cursor::new((response.body)()),
            response.body_len,
            None,
        )
    }
}

