use crate::io::Text;
use crate::Error;
use futures_io::AsyncRead;
use http::Response;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

/// Provides extension methods for working with HTTP responses.
pub trait ResponseExt<T> {
    /// Copy the response body into a writer.
    ///
    /// Returns the number of bytes that were written.
    fn copy_to(&mut self, writer: impl Write) -> io::Result<u64>
    where
        T: Read;

    /// Write the response body to a file.
    ///
    /// This method makes it convenient to download a file using a GET request
    /// and write it to a file synchronously in a single chain of calls.
    ///
    /// Returns the number of bytes that were written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// isahc::get("https://httpbin.org/image/jpeg")?
    ///     .copy_to_file("myimage.jpg")?;
    /// # Ok::<(), isahc::Error>(())
    /// ```
    fn copy_to_file(&mut self, path: impl AsRef<Path>) -> io::Result<u64>
    where
        T: Read,
    {
        File::create(path).and_then(|f| self.copy_to(f))
    }

    /// Get the response body as a string.
    ///
    /// This method consumes the entire response body stream and can only be
    /// called once, unless you can rewind this response body.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    ///
    /// let text = isahc::get("https://example.org")?.text()?;
    /// println!("{}", text);
    /// # Ok::<(), isahc::Error>(())
    /// ```
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

    /// Deserialize the response body as JSON into a given type.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use isahc::prelude::*;
    /// use serde_json::Value;
    ///
    /// let json: Value = isahc::get("https://httpbin.org/json")?.json()?;
    /// println!("author: {}", json["slideshow"]["author"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "json")]
    fn json<D>(&mut self) -> Result<D, serde_json::Error>
    where
        D: serde::de::DeserializeOwned,
        T: Read;
}

impl<T> ResponseExt<T> for Response<T> {
    fn copy_to(&mut self, mut writer: impl Write) -> io::Result<u64>
    where
        T: Read,
    {
        io::copy(self.body_mut(), &mut writer)
    }

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

    #[cfg(feature = "json")]
    fn json<D>(&mut self) -> Result<D, serde_json::Error>
    where
        D: serde::de::DeserializeOwned,
        T: Read,
    {
        serde_json::from_reader(self.body_mut())
    }
}
