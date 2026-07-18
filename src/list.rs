use curl_sys::{curl_slist, curl_slist_append, curl_slist_free_all};
use std::{
    ffi::{CStr, CString, NulError},
    fmt,
    ptr::null_mut,
    sync::Arc,
};

/// Alternative safe wrapper for a curl slist, a linked list of a strings.
///
/// This is used as an alternative to the safe wrapper provided by curl-rust,
/// which does not support reusing the same list across multiple requests. Due
/// to how Isahc manages request configuration and cloning this can be pretty
/// wasteful.
///
/// This version mandates reference counting to allow automatic memory sharing
/// across requests for the same list. The compromise this makes is that the
/// list is immutable once built, but this is acceptable for our use case.
#[derive(Clone)]
pub(crate) struct ArcList(Arc<SList>);

impl ArcList {
    /// Create a new list containing only a single item.
    pub(crate) fn singleton(string: impl AsRef<CStr>) -> Self {
        Self::builder().append(string).build()
    }

    /// Create a new list builder.
    pub(crate) fn builder() -> Builder {
        Builder::default()
    }

    /// Get the underlying raw pointer of the list.
    pub(crate) fn as_raw_ptr(&self) -> *mut curl_slist {
        self.0.raw
    }

    /// Create an iterator for walking forward through the items in the list.
    #[allow(unused)]
    pub(crate) fn iter(&self) -> Iter {
        Iter {
            _list: self,
            head: self.0.raw,
        }
    }
}

impl TryFrom<String> for ArcList {
    type Error = NulError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = CString::new(value)?;

        Ok(Self::singleton(value))
    }
}

impl fmt::Debug for ArcList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArcList").finish()
    }
}

/// Builder for an [`ArcList`]. Since the type is immutable a builder is
/// required to create a list with items.
#[derive(Default)]
pub(crate) struct Builder(SList);

impl Builder {
    /// Append a string to the list. The list holds C strings, so a C string is
    /// required.
    pub(crate) fn append(mut self, string: impl AsRef<CStr>) -> Self {
        let raw = unsafe { curl_slist_append(self.0.raw, string.as_ref().as_ptr()) };
        assert!(!raw.is_null());
        self.0.raw = raw;
        self
    }

    /// Build the list. The returned list is immutable, and all strings added to
    /// this builder are moved into the list.
    pub(crate) fn build(self) -> ArcList {
        ArcList(Arc::new(self.0))
    }
}

/// Iterates over items in a list.
pub(crate) struct Iter<'a> {
    _list: &'a ArcList,
    head: *mut curl_slist,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a CStr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                let item = self.head.read();
                let cstr = CStr::from_ptr(item.data);
                self.head = item.next;
                Some(cstr)
            }
        }
    }
}

/// Wrapper around an slist pointer that adds a destructor.
struct SList {
    raw: *mut curl_slist,
}

impl SList {
    /// Create a new empty list. Curl doesn't have a special representation for
    /// empty lists; null just means an empty list.
    const fn new() -> Self {
        Self { raw: null_mut() }
    }
}

impl Default for SList {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SList {
    fn drop(&mut self) {
        // According to curl docs, passing in a null pointer here does nothing.
        unsafe { curl_slist_free_all(self.raw) }
    }
}

unsafe impl Send for SList {}
unsafe impl Sync for SList {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_creates_one_item() {
        let list = ArcList::singleton(CStr::from_bytes_with_nul(b"hello\0").unwrap());
        let items: Vec<&CStr> = list.iter().collect();
        assert_eq!(items, vec![CStr::from_bytes_with_nul(b"hello\0").unwrap()]);
    }

    #[test]
    fn builder_appends_and_builds() {
        let list = Builder::default()
            .append(CString::new("first").expect("valid CString"))
            .append(CString::new("second").expect("valid CString"))
            .build();

        let items: Vec<&CStr> = list.iter().collect();
        assert_eq!(
            items,
            vec![
                CStr::from_bytes_with_nul(b"first\0").unwrap(),
                CStr::from_bytes_with_nul(b"second\0").unwrap(),
            ]
        );
    }

    #[test]
    fn try_from_string_creates_singleton() {
        let s = String::from("world");
        let list = ArcList::try_from(s).unwrap();
        let items: Vec<&CStr> = list.iter().collect();
        assert_eq!(items, vec![CStr::from_bytes_with_nul(b"world\0").unwrap()]);
    }

    #[test]
    fn iter_empty_yields_none() {
        let list = Builder::default().build();
        let mut iter = list.iter();
        assert!(iter.next().is_none());
    }

    #[test]
    fn cloned_list_has_same_items() {
        let original = Builder::default()
            .append(CString::new("a").unwrap())
            .append(CString::new("b").unwrap())
            .build();

        let cloned = original.clone();

        let orig_items: Vec<&CStr> = original.iter().collect();
        let clone_items: Vec<&CStr> = cloned.iter().collect();

        assert_eq!(orig_items, clone_items);
    }
}
