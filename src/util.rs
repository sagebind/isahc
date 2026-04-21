use curl::easy::Easy2;
use curl_sys::{CURL_BLOB_NOCOPY, CURLE_OK, CURLoption, curl_blob};

/// Set a curl option to a blob without copying the data.
///
/// # Safety
///
/// The caller must ensure that the data slice remains valid and at a stable
/// memory address until the easy handle is destroyed or a different blob is
/// set.
pub(crate) unsafe fn set_blob_nocopy<H>(
    easy: &mut Easy2<H>,
    option: CURLoption,
    data: &[u8],
) -> Result<(), curl::Error> {
    let blob = curl_blob {
        data: data.as_ptr().cast_mut().cast(),
        len: data.len(),
        flags: CURL_BLOB_NOCOPY,
    };

    let code = unsafe { curl_sys::curl_easy_setopt(easy.raw(), option, &blob) };

    if code == CURLE_OK {
        Ok(())
    } else {
        let mut err = curl::Error::new(code);

        if let Some(msg) = easy.take_error_buf() {
            err.set_extra(msg);
        }

        Err(err)
    }
}
