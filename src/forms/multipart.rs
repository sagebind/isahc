use std::{collections::VecDeque, convert::TryFrom, io, io::{Write, Read}, iter::repeat_with, pin::Pin, task::{Context, Poll}};

use futures_lite::{
    io::{Chain, Cursor},
    ready,
    AsyncRead,
    AsyncReadExt,
};
use http::header::{HeaderValue, HeaderName};

use crate::body::{Body, AsyncBody};

/// Builder for constructing a multipart form.
///
/// Generates a multipart form body as described in [RFC
/// 7578](https://datatracker.ietf.org/doc/html/rfc7578).
#[derive(Debug)]
pub struct FormDataBuilder<BODY> {
    boundary: String,
    fields: Vec<FormPart<BODY>>,
}

impl<BODY> FormDataBuilder<BODY> {
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
        V: Into<BODY>,
    {
        self.part(FormPart::new(name, value))
    }

    /// Append a part to this form.
    pub fn part(mut self, part: FormPart<BODY>) -> Self {
        self.fields.push(part);
        self
    }
}

impl FormDataBuilder<Body> {
    /// Build the form.
    pub fn build(self) -> Body {
        let boundary = self.boundary;

        let mut parts = VecDeque::with_capacity(self.fields.len());
        let mut len = Some(boundary.len() as u64);

        for part in self.fields {
            let (read, part_size) = part.into_read(boundary.as_str());
            parts.push_back(read);

            if let (Some(a), Some(b)) = (len, part_size) {
                len = Some(a + b);
            }
            else {
                len = None;
            }
        }

        let terminator = std::io::Cursor::new(format!("--{}--\r\n", &boundary).into_bytes());

        len = len.map(|size| size + terminator.get_ref().len() as u64);

        let parts = MultiChain {
            items: parts,
        };

        let full_reader = parts.chain(terminator);

        let mut body = if let Some(len) = len {
            Body::from_reader_sized(full_reader, len)
        } else {
            Body::from_reader(full_reader)
        };

        body = body.with_content_type(Some(format!("multipart/form-data; boundary={}", boundary).parse().unwrap()));

        body
    }
}

impl FormDataBuilder<AsyncBody> {
    /// Build the form.
    pub fn build(self) -> AsyncBody {
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

        let parts = MultiChain {
            items: parts,
        };

        let full_reader = parts.chain(terminator);

        let mut body = if let Some(len) = len {
            AsyncBody::from_reader_sized(full_reader, len)
        } else {
            AsyncBody::from_reader(full_reader)
        };

        body = body.with_content_type(Some(format!("multipart/form-data; boundary={}", boundary).parse().unwrap()));

        body
    }
}

/// A single part of a multipart form representing a single field.
#[derive(Debug)]
pub struct FormPart<BODY> {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    headers: Vec<(HeaderName, HeaderValue)>,
    value: BODY,
    error: Option<http::Error>,
}

impl<BODY> FormPart<BODY> {
    /// Create a new form part with a name and value.
    pub fn new<N, V>(name: N, value: V) -> Self
    where
        N: Into<String>,
        V: Into<BODY>,
    {
        FormPart {
            name: name.into(),
            filename: None,
            content_type: Some(String::from("text/plain;charset=UTF-8")),
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
}

impl FormPart<Body> {
    fn into_read(self, boundary: &str) -> (impl Read, Option<u64>) {
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

        let reader = std::io::Cursor::new(header).chain(self.value).chain(std::io::Cursor::new(b"\r\n"));

        (reader, None)
    }
}

impl FormPart<AsyncBody> {
    // Chain<Chain<Cursor<Vec<u8>>, AsyncBody>, &'static [u8]>
    fn into_writer(self, boundary: &str) -> Chain<Chain<Cursor<Vec<u8>>, AsyncBody>, &'static [u8]> {
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

/// A chained reader which can chain multiple readers of the same type together.
struct MultiChain<R> {
    items: VecDeque<R>,
}

impl<R: Read> Read for MultiChain<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while let Some(item) = self.items.front_mut() {
            match item.read(buf) {
                Ok(0) => {
                    // This item has finished being read, discard it and move to
                    // the next one.
                    self.items.pop_front();
                }
                result => return result,
            }
        }

        Ok(0)
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for MultiChain<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        while let Some(item) = self.items.front_mut() {
            match ready!(AsyncRead::poll_read(Pin::new(item), cx, buf)) {
                Ok(0) => {
                    // This item has finished being read, discard it and move to
                    // the next one.
                    self.items.pop_front();
                }
                result => return Poll::Ready(result),
            }
        }

        Poll::Ready(Ok(0))
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
        let mut form: AsyncBody = FormDataBuilder::<AsyncBody>::with_boundary("boundary").build();

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
        let mut form: AsyncBody = FormDataBuilder::<AsyncBody>::with_boundary("boundary")
            .field("foo", "value1")
            .field("bar", "value2")
            .build();

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
