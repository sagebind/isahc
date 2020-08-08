mod endpoint;
mod request;
mod response;
mod server;

pub use endpoint::Endpoint;
pub use request::Request;
pub use response::Response;

#[doc(hidden)]
pub use paste::paste;

#[macro_export]
macro_rules! endpoint {
    (@builder($builder:ident) headers {
        $(
            $name:tt: $value:expr,
        )*
    } $($tail:tt)*) => {
        $(
            let $builder = $builder.header($name, $value);
        )*

        $crate::endpoint!(@builder($builder) $($tail)*);
    };

    (@builder($builder:ident) $prop:ident: |$($args:ident),*| {$($body:tt)*}, $($tail:tt)*) => {
        $crate::paste! {
            let $builder = $builder.[<$prop _fn>](|$($args)*| {$($body)*});
        }
        $crate::endpoint!(@builder($builder) $($tail)*)
    };

    (@builder($builder:ident) $prop:ident: $value:expr, $($tail:tt)*) => {
        let $builder = $builder.$prop($value);
        $crate::endpoint!(@builder($builder) $($tail)*)
    };

    (@builder($builder:ident)) => {};

    ($($inner:tt)*) => {{
        let builder = $crate::Endpoint::builder();

        $crate::endpoint!(@builder(builder) $($inner)*);

        builder.build()
    }};
}
