mod mock;
mod request;
mod responder;
mod response;

pub mod socks4;

pub use mock::Mock;
pub use request::Request;
pub use responder::Responder;
pub use response::Response;

/// Macro to define a mock endpoint using a more concise DSL.
#[macro_export]
macro_rules! mock {
    (@response($response:expr) status: $status:expr, $($tail:tt)*) => {{
        let mut response = $response;

        response.status_code = $status as u16;

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) body: $body:expr, $($tail:tt)*) => {{
        let mut response = $response;

        response.body = $body.into();

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) transfer_encoding: $value:expr, $($tail:tt)*) => {{
        let mut response = $response;

        response.transfer_encoding = $value;

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) delay: $delay:tt, $($tail:tt)*) => {{
        let duration = $crate::helpers::parse_duration(stringify!($delay));
        ::std::thread::sleep(duration);

        $crate::mock!(@response($response) $($tail)*)
    }};

    (@response($response:expr) headers {
        $(
            $name:literal: $value:expr,
        )*
    } $($tail:tt)*) => {{
        let mut response = $response;

        $(
            response.headers.push(($name.to_string(), $value.to_string()));
        )*

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr)) => {{
        $response
    }};

    ($($inner:tt)*) => {{
        struct Responder<F>(F);

        impl<F> $crate::Responder for Responder<F>
        where
            F: Send + Sync + 'static + Fn($crate::Request) -> Option<$crate::Response>,
        {
            fn respond(&self, request: $crate::Request) -> Option<$crate::Response> {
                (self.0)(request)
            }
        }

        $crate::Mock::new(Responder(move |request| {
            let mut response = $crate::Response::default();

            let response = $crate::mock!(@response(response) $($inner)*);

            Some(response)
        }))
    }};
}

#[doc(hidden)]
pub mod helpers {
    use std::time::Duration;

    pub fn parse_duration(s: &str) -> Duration {
        humantime::parse_duration(s).unwrap()
    }
}
