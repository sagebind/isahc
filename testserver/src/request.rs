use regex::Regex;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub(crate) number: u32,
    pub(crate) method: String,
    pub(crate) url: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Option<Vec<u8>>,
}

impl Request {
    pub fn method(&self) -> &str {
        self.method.as_str()
    }

    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    /// Get the request number.
    ///
    /// This is a monotonically increasing number, starting from 0, that
    /// indicates the order of requests received by the mock.
    pub fn number(&self) -> u32 {
        self.number
    }

    pub fn get_header(&self, name: impl AsRef<str>) -> impl Iterator<Item = String> + '_ {
        let name_lower = name.as_ref().to_lowercase();

        self.headers
            .iter()
            .filter(move |(name, _)| name.to_lowercase() == name_lower)
            .map(|(_, value)| value.clone())
    }

    pub fn expect_header(&self, name: impl AsRef<str>, value: impl AsRef<str>) {
        self.expect_header_matches(name, |v| v == value.as_ref())
    }

    pub fn expect_header_regex(&self, name: impl AsRef<str>, regex: &str) {
        let regex = Regex::new(regex).unwrap();
        self.expect_header_matches(name, |v| regex.is_match(v))
    }

    pub fn expect_header_matches(&self, name: impl AsRef<str>, predicate: impl Fn(&str) -> bool) {
        let name = name.as_ref();

        self.get_header(name)
            .find(|v| predicate(v))
            .unwrap_or_else(|| panic!("no header named `{}` with value expected found", name));
    }

    pub fn expect_body(&self, expected: impl AsRef<[u8]>) {
        if let Some(body) = self.body.as_ref() {
            assert_eq!(expected.as_ref(), body.as_slice());
        } else {
            panic!("expected a body, but request had none");
        }
    }
}
