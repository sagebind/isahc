use std::ascii;

// pub mod agent;
pub mod handler;
pub mod notify;
pub mod parse;
// pub mod request;
pub mod response;

pub fn format_byte_string(bytes: impl AsRef<[u8]>) -> String {
    String::from_utf8(bytes
        .as_ref()
        .iter()
        .flat_map(|byte| ascii::escape_default(*byte))
        .collect())
        .unwrap_or(String::from("<binary>"))
}
