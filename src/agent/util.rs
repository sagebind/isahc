use std::hash::Hasher;

/// Trivial hash function to use for our maps and sets that use file descriptors
/// as keys.
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

            if bytes.len() > size_of::<u64>() {
                panic!("only a maximum of 8 bytes can be hashed");
            }
        }

        (&mut self.0[..bytes.len()]).copy_from_slice(bytes);
    }

    #[inline]
    fn finish(&self) -> u64 {
        u64::from_ne_bytes(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::IntHasher;
    use curl::multi::Socket;
    use quickcheck_macros::quickcheck;
    use std::hash::{Hash, Hasher};

    #[test]
    #[should_panic]
    fn hash_of_more_than_8_bytes_panics() {
        "hello".hash(&mut IntHasher::default());
    }

    #[quickcheck]
    fn hash_socket_is_deterministic(socket: Socket) {
        let mut hasher_a = IntHasher::default();
        socket.hash(&mut hasher_a);

        let mut hasher_b = IntHasher::default();
        socket.hash(&mut hasher_b);

        assert_eq!(hasher_a.finish(), hasher_b.finish());
    }

    #[quickcheck]
    fn hash_of_two_sockets_are_equal_iff_sockets_are_equal(a: Socket, b: Socket) {
        let mut hasher_a = IntHasher::default();
        a.hash(&mut hasher_a);

        let mut hasher_b = IntHasher::default();
        b.hash(&mut hasher_b);

        if a == b {
            assert_eq!(hasher_a.finish(), hasher_b.finish());
        } else {
            assert_ne!(hasher_a.finish(), hasher_b.finish());
        }
    }
}
