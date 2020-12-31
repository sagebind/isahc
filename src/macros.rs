/// Helper macro that allows you to attempt to downcast a generic type, as long
/// as it is known to be `'static`.
macro_rules! match_type {
    {
        $(
            <$name:ident as $T:ty> => $branch:expr,
        )*
        $defaultName:ident => $defaultBranch:expr,
    } => {{
        match () {
            $(
                _ if ::std::any::Any::type_id(&$name) == ::std::any::TypeId::of::<$T>() => {
                    #[allow(unsafe_code)]
                    let $name: $T = unsafe {
                        ::std::mem::transmute_copy::<_, $T>(&::std::mem::ManuallyDrop::new($name))
                    };
                    $branch
                }
            )*
            _ => $defaultBranch,
        }
    }};
}

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
            pub struct $ident<'a $($(, $T)*)*> {
                inner: ::std::pin::Pin<Box<dyn ::std::future::Future<Output = $output> + 'a>>,
                $($($T: ::std::marker::PhantomData<$T>,)*)*
            }

            impl<'a, $($($T)*)*> $ident<'a, $($($T)*)*> {
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

            impl<$($($T: Unpin)*)*> ::std::future::Future for $ident<'_, $($($T)*)*> {
                type Output = $output;

                fn poll(mut self: ::std::pin::Pin<&mut Self>, cx: &mut ::std::task::Context<'_>) -> ::std::task::Poll<Self::Output> {
                    self.as_mut().inner.as_mut().poll(cx)
                }
            }

            $(
                #[allow(unsafe_code)]
                unsafe impl<$($S: Send),*> Send for $ident<'_, $($S)*> {}
            )*
        )*
    };
}
