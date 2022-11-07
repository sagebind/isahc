use std::ops::Deref;

pub(crate) mod future;
pub(crate) mod io;
pub(crate) mod task;

enum MaybeOwned<T, B> {
    Owned(T),
    Borrowed(B),
}

impl<T, B> Deref for MaybeOwned<T, B>
where
    B: Deref<Target = T>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(ref value) => value,
            Self::Borrowed(borrow) => borrow.deref(),
        }
    }
}
