macro_rules! decl_future {
    (
        $(
            $(#[$meta:meta])*
            $vis:vis type $ident:ident$(<$($T:ident),*>)? = impl Future<Output = $output:ty> $(+ SendIf<$($S:ident),+>)?;
        )*
    ) => {
        $(
            $(#[$meta])*
            #[allow(missing_debug_implementations, non_snake_case)]
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            $vis struct $ident<'a, $($($T),*)*> {
                inner: ::std::pin::Pin<Box<dyn ::std::future::Future<Output = $output> + 'a>>,
                $($($T: ::std::marker::PhantomData<$T>,)*)*
            }

            $(#[$meta])*
            impl<'a, $($($T),*)*> $ident<'a, $($($T),*)*> {
                pub(crate) fn new<F>(future: F) -> Self
                where
                F: ::std::future::Future<Output = $output> + 'a,
                {
                    Self {
                        inner: Box::pin(future),
                        $($($T: ::std::marker::PhantomData,)*)*
                    }
                }
            }

            $(#[$meta])*
            impl<$($($T: Unpin),*)*> ::std::future::Future for $ident<'_, $($($T),*)*> {
                type Output = $output;

                fn poll(mut self: ::std::pin::Pin<&mut Self>, cx: &mut ::std::task::Context<'_>) -> ::std::task::Poll<Self::Output> {
                    self.as_mut().inner.as_mut().poll(cx)
                }
            }

            $(#[$meta])*
            $(
                #[allow(unsafe_code)]
                unsafe impl<$($S: Send),*> Send for $ident<'_, $($S),*> {}
            )*
        )*
    };
}
