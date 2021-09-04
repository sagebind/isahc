use std::{collections::VecDeque, convert::TryFrom, io, io::Write, iter::repeat_with, pin::Pin, task::{Context, Poll}};

use futures_lite::{
    io::{Chain, Cursor},
    ready,
    AsyncRead,
    AsyncReadExt,
};
use http::header::{HeaderValue, HeaderName};

use crate::AsyncBody;

type PartReader = Chain<Chain<Cursor<Vec<u8>>, AsyncBody>, &'static [u8]>;

/// Builder for constructing a multipart form.
///
/// Generates a multipart form body as described in [RFC
/// 7578](https://datatracker.ietf.org/doc/html/rfc7578).
#[derive(Debug)]
pub struct FormDataBuilder {
    boundary: String,
    fields: Vec<FormPart>,
}

impl FormDataBuilder {
    /// Create a new form builder.
    pub fn new() -> Self {
        Self::with_boundary(generate_boundary())
    }

    /// Specify a boundary to use. A random one will be generated if one is not
    /// specified.
    fn with_boundary<S: Into<String>>(boundary: S) -> Self {
        Self {
            boundary: boundary.into(),
            fields: Vec::new(),
        }
    }

    /// Append a field to this form with a given name and value.
    ///
    /// Duplicate fields with the same name are allowed and will be preserved in
    /// the order they are added.
    pub fn field<N, V>(self, name: N, value: V) -> Self
    where
        N: Into<String>,
        V: Into<AsyncBody>,
    {
        self.part(FormPart::new(name, value))
    }

    /// Append a part to this form.
    pub fn part(mut self, part: FormPart) -> Self {
        self.fields.push(part);
        self
    }

    /// Build the form.
    pub fn build(self) -> FormData {
        let boundary = self.boundary;

        let parts = self
            .fields
            .into_iter()
            .map(|field| field.into_writer(boundary.as_str()))
            .collect::<VecDeque<_>>();

        let terminator = Cursor::new(format!("--{}--\r\n", &boundary).into_bytes());

        // Try to compute the size of the body we will write. This can only be
        // determined if all parts contain values that are also sized.
        let len = parts
            .iter()
            .map(|part| {
                Some(
                    part.get_ref().0.get_ref().1.len()?
                        + part.get_ref().0.get_ref().0.get_ref().len() as u64
                        + part.get_ref().1.len() as u64,
                )
            })
            .fold(Some(0), |a, b| Some(a? + b?))
            .map(|size| size + terminator.get_ref().len() as u64);

        FormData {
            len,
            parts,
            terminator,
        }
    }
}

/// A single part of a multipart form representing a single field.
#[derive(Debug)]
pub struct FormPart {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    headers: Vec<(HeaderName, HeaderValue)>,
    value: AsyncBody,
    error: Option<http::Error>,
}

impl FormPart {
    /// Create a new form part with a name and value.
    pub fn new<N, V>(name: N, value: V) -> Self
    where
        N: Into<String>,
        V: Into<AsyncBody>,
    {
        FormPart {
            name: name.into(),
            filename: None,
            content_type: None,
            headers: Vec::new(),
            value: value.into(),
            error: None,
        }
    }

    /// Set the filename of this form part.
    pub fn filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }

    /// Set the content type of this form part.
    pub fn content_type(mut self, content_type: String) -> Self {
        self.content_type = Some(content_type);
        self
    }

    /// Append a custom header to this form part.
    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        if self.error.is_none() {
            let name = <HeaderName as TryFrom<K>>::try_from(name).map_err(Into::into).unwrap();
            let value = <HeaderValue as TryFrom<V>>::try_from(value).map_err(Into::into).unwrap();

            self.headers.push((name, value));
        }

        self
    }

    fn into_writer(self, boundary: &str) -> PartReader {
        let mut header = Vec::new();

        write!(header, "--{}\r\n", boundary).unwrap();
        write!(
            header,
            "Content-Disposition: form-data; name=\"{}\"",
            &self.name
        )
        .unwrap();

        if let Some(filename) = self.filename.as_ref() {
            write!(header, "; filename=\"{}\"", filename).unwrap();
        }

        header.extend_from_slice(b"\r\n");

        if let Some(content_type) = self.content_type.as_ref() {
            write!(header, "Content-Type: {}\r\n", content_type).unwrap();
        }

        for (name, value) in self.headers {
            header.extend_from_slice(name.as_ref());
            header.extend_from_slice(b": ");
            header.extend_from_slice(value.as_ref());
            header.extend_from_slice(b"\r\n");
        }

        header.extend_from_slice(b"\r\n");

        Cursor::new(header).chain(self.value).chain(b"\r\n")
    }
}

/// A multipart form body.
#[derive(Debug)]
pub struct FormData {
    len: Option<u64>,
    parts: VecDeque<PartReader>,
    terminator: Cursor<Vec<u8>>,
}

impl From<FormData> for AsyncBody {
    fn from(form: FormData) -> Self {
        if let Some(len) = form.len {
            AsyncBody::from_reader_sized(form, len)
        } else {
            AsyncBody::from_reader(form)
        }
    }
}

impl AsyncRead for FormData {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        while let Some(part) = self.parts.front_mut() {
            match ready!(AsyncRead::poll_read(Pin::new(part), cx, buf)) {
                Ok(0) => {
                    // This part has finished being read, discard it and move to
                    // the next one.
                    self.parts.pop_front();
                }
                result => return Poll::Ready(result),
            }
        }

        AsyncRead::poll_read(Pin::new(&mut self.terminator), cx, buf)
    }
}

fn generate_boundary() -> String {
    repeat_with(fastrand::alphanumeric).take(24).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::{future::block_on, io::AsyncReadExt};

    #[test]
    fn empty_form() {
        let mut form: AsyncBody = FormDataBuilder::with_boundary("boundary").build().into();

        let expected = "--boundary--\r\n";

        assert_eq!(form.len(), Some(expected.len() as u64));

        let contents = block_on(async {
            let mut buf = String::new();
            form.read_to_string(&mut buf).await.unwrap();
            buf
        });

        assert_eq!(contents, expected);
    }

    #[test]
    fn sized_form() {
        let mut form: AsyncBody = FormDataBuilder::with_boundary("boundary")
            .field("foo", "value1")
            .field("bar", "value2")
            .build()
            .into();

        let expected = "\
            --boundary\r\n\
            Content-Disposition: form-data; name=\"foo\"\r\n\
            \r\n\
            value1\r\n\
            --boundary\r\n\
            Content-Disposition: form-data; name=\"bar\"\r\n\
            \r\n\
            value2\r\n\
            --boundary--\r\n\
        ";

        let contents = block_on(async {
            let mut buf = String::new();
            form.read_to_string(&mut buf).await.unwrap();
            buf
        });

        assert_eq!(contents, expected);
        assert_eq!(form.len(), Some(expected.len() as u64));
    }
}
