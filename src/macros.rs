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
