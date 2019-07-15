use crate::io::Text;
use crate::Error;
use futures::io::AsyncRead;
use http::Response;
use std::io::Read;

/// Provides extension methods for working with HTTP responses.
pub trait ResponseExt<T> {
    /// Get the response body as a string.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
    fn text(&mut self) -> Result<String, Error>
    where
        T: Read;

    /// Get the response body as a string asynchronously.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
    fn text_async(&mut self) -> Text<'_, T>
    where
        T: AsyncRead + Unpin;
}

impl<T> ResponseExt<T> for Response<T> {
    fn text(&mut self) -> Result<String, Error>
    where
        T: Read,
    {
        let mut s = String::default();
        self.body_mut().read_to_string(&mut s)?;
        Ok(s)
    }

    fn text_async(&mut self) -> Text<'_, T>
    where
        T: AsyncRead + Unpin,
    {
        Text::new(self.body_mut())
    }
}
