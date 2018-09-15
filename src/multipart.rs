//! Helpers for creating multi-part form requests.

use body::Body;
use mime::Mime;
use std::collections::HashMap;
use std::io::{Chain, Cursor};

pub struct FormBuilder {
    parts: HashMap<String, FormPart>,
}

pub struct FormPart {
    name: String,
    content_type: Mime,
    body: Body,
}

impl FormBuilder {
    pub fn build(self) -> Body {
        let mut readers = Vec::new();

        for (_, part) in self.parts {
            readers.push(part.body);
        }

        Body::default()
    }
}
