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
pub(crate) struct ArcList(Arc<Inner>);

struct Inner {
    raw: *mut curl_slist,
}

impl ArcList {
    pub(crate) fn singleton(string: impl Into<Vec<u8>>) -> Result<Self, NulError> {
        Ok(Self::builder().append(string)?.build())
    }

    pub(crate) fn builder() -> Builder {
        Builder::default()
    }

    pub(crate) fn as_raw_ptr(&self) -> *mut curl_slist {
        self.0.raw
    }

    pub(crate) fn iter(&self) -> Iter {
        Iter {
            list: self,
            head: self.0.raw,
        }
    }
}

impl fmt::Debug for ArcList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArcList").finish()
    }
}

unsafe impl Send for ArcList {}
unsafe impl Sync for ArcList {}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe { curl_slist_free_all(self.raw) }
    }
}

pub(crate) struct Builder {
    raw: *mut curl_slist,
}

impl Default for Builder {
    fn default() -> Self {
        Self { raw: null_mut() }
    }
}

impl Builder {
    pub(crate) fn append(mut self, string: impl Into<Vec<u8>>) -> Result<Self, NulError> {
        let string = CString::new(string)?;
        let raw = unsafe { curl_slist_append(self.raw, string.as_ptr()) };
        assert!(!raw.is_null());
        self.raw = raw;
        Ok(self)
    }

    pub(crate) fn build(mut self) -> ArcList {
        let raw = self.raw;
        self.raw = null_mut();
        ArcList(Arc::new(Inner { raw }))
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        unsafe { curl_slist_free_all(self.raw) }
    }
}

pub(crate) struct Iter<'a> {
    list: &'a ArcList,
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
