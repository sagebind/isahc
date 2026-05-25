use curl_sys::{CURL_BLOB_NOCOPY, curl_blob};
use std::{cell::UnsafeCell, fmt, ptr, sync::Arc};

/// A heap allocated, reference-counted object containing binary data, which can
/// be shared with libcurl without copying.
///
/// Cloning this struct is cheap and only increments the reference count to the
/// underlying data.
#[derive(Clone)]
pub(crate) struct Blob {
    arc: Arc<Inner<dyn AsRef<[u8]> + Send + Sync + 'static>>,
}

/// The inner struct stored on the heap. This struct will be dynamically sized
/// so that the trait object and blob can be stored together without extra
/// indirection.
///
/// This is generic to work around a current Rust limitation that prevents us
/// from instantiating this struct when a literal trait object is specified as a
/// field type.
struct Inner<T: ?Sized> {
    /// The raw blob struct we pass to libcurl.
    ///
    /// # Safety
    ///
    /// This contains a pointer to data owned by the `object` field. This field
    /// therefore must be dropped first, which is why it is defined first in the
    /// struct.
    blob: UnsafeCell<curl_blob>,

    /// The underlying object. We only hold onto this to maintain ownership of
    /// the data, and to invoke its drop handler if it has one when we are done
    /// with the blob.
    object: T,
}

impl Blob {
    /// Allocate a new blob containing the given bytes. Any type can be used
    /// that represents or wraps a contiguous byte slice. The bytes will not be
    /// copied, but instead shared with libcurl directly.
    pub(crate) fn new<B: AsRef<[u8]> + Send + Sync + 'static>(bytes: B) -> Self {
        // First place the object into the `Arc` to give it a stable memory
        // location.
        let arc = Arc::new(Inner {
            // Fill the blob with empty data. We'll update these in a bit.
            blob: UnsafeCell::new(curl_blob {
                data: ptr::null_mut(),
                len: 0,
                flags: CURL_BLOB_NOCOPY,
            }),
            object: bytes,
        });

        // Now we can get a reference to the bytes and construct the blob, now
        // that it is on the heap.
        let slice = arc.object.as_ref();

        // SAFETY: We assume that the `AsRef` implementation will return the
        // same slice on every call. We call it a single time, and "cache" the
        // results so that we can construct the blob once and reuse it.
        //
        // Usually this is the case, but even if it is not, the type signature
        // of `AsRef` guarantees that this initial slice we receive will be
        // valid for the lifetime of the object. While this won't cause any
        // undefined behavior, it *may* cause unexpected behavior if the caller
        // for some reason thought that they could keep returning different byte
        // slices and we would actually respect that in real time.
        unsafe {
            arc.blob.get().write(curl_blob {
                data: slice.as_ptr().cast_mut().cast(),
                len: slice.len(),
                flags: CURL_BLOB_NOCOPY,
            });
        }

        Self { arc }
    }

    /// Get the length of the blob in bytes.
    pub(crate) fn len(&self) -> usize {
        self.as_raw_ref().len
    }

    /// Get a pointer to the raw blob struct that can be passed to libcurl.
    pub(crate) fn as_raw_ptr(&self) -> *const curl_blob {
        self.arc.blob.get()
    }

    fn as_raw_ref(&self) -> &curl_blob {
        // SAFETY: The blob is initialized in the constructor and never modified
        // after that, and we never give out mutable references to it.
        unsafe { &*self.as_raw_ptr() }
    }
}

impl fmt::Debug for Blob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Blob").field("len", &self.len()).finish()
    }
}

/// # Safety
///
/// The inner pointer is `Send` as long as the object it references is `Send`.
unsafe impl<T: Send + ?Sized> Send for Inner<T> {}

/// # Safety
///
/// The inner pointer is `Sync` as long as the object it references is `Sync`.
unsafe impl<T: Sync + ?Sized> Sync for Inner<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_len_matches_input() {
        let data = vec![1, 2, 3, 4, 5];
        let blob = Blob::new(data);
        assert_eq!(blob.len(), 5);
    }

    #[test]
    fn blob_has_expected_flags() {
        let data = vec![1, 2, 3, 4, 5];
        let blob = Blob::new(data);
        assert_eq!(blob.as_raw_ref().flags, CURL_BLOB_NOCOPY);
    }

    #[test]
    fn blob_from_stack_array() {
        let data = [1, 2, 3];
        let blob = Blob::new(data);
        assert_eq!(blob.len(), 3);
    }

    #[test]
    fn blob_clone_preserves_data_and_len() {
        let data = "hello world";
        let len = data.len();
        let blob1 = Blob::new(data);
        let blob2 = blob1.clone();

        assert_eq!(blob1.len(), len);
        assert_eq!(blob2.len(), len);

        let raw1 = blob1.as_raw_ref();
        let raw2 = blob2.as_raw_ref();

        assert!(!raw1.data.is_null());
        assert_eq!(raw1.data, raw2.data);
        assert_eq!(raw1.len, raw2.len);
        assert_eq!(raw1.flags, raw2.flags);
    }
}
