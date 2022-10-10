use std::hash::{BuildHasherDefault, Hasher};

pub(super) type BuildIntHasher = BuildHasherDefault<IntHasher>;

/// Trivial hash function to use for our maps and sets that use simple integer
/// types or file descriptors as keys.
#[derive(Default)]
pub(super) struct IntHasher([u8; 8], #[cfg(debug_assertions)] bool);

impl Hasher for IntHasher {
    fn write(&mut self, bytes: &[u8]) {
        #[cfg(debug_assertions)]
        {
            if self.1 {
                panic!("socket hash function can only be written to once");
            } else {
                self.1 = true;
            }

            if bytes.len() > 8 {
                panic!("only a maximum of 8 bytes can be hashed");
            }
        }

        (self.0[..bytes.len()]).copy_from_slice(bytes);
    }

    #[inline]
    fn finish(&self) -> u64 {
        u64::from_ne_bytes(self.0)
    }
}
