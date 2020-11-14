use http::HeaderMap;
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// Holds the current state of a trailer for a response.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trailer {
    headers: Arc<OnceCell<HeaderMap>>,
}

impl Trailer {
    pub(crate) fn headers(&self) -> Option<&HeaderMap> {
        self.headers.get()
    }

    pub(crate) fn flush(&self, headers: HeaderMap) {
        if self.headers.set(headers).is_err() {
            tracing::warn!("tried to flush trailer multiple times");
        }
    }
}
