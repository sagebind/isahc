//! HTTP server for testing.

#[macro_use]
mod macros;
mod mock;
mod pool;
mod request;
mod responder;
mod response;

pub mod socks4;

pub use macros::macro_api;
pub use mock::Mock;
pub use request::Request;
pub use responder::{RequestContext, Responder};
pub use response::Response;

/// Macro to define a mock endpoint using a more concise DSL.
#[macro_export]
macro_rules! mock_pld {
    (@response($response:expr) status: $status:expr, $($tail:tt)*) => {{
        let mut response = $response;

        response.status_code = $status as u16;

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) body: $body:expr, $($tail:tt)*) => {{
        let mut response = $response;

        response = response.with_body_buf($body);

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) body_reader: $body:expr, $($tail:tt)*) => {{
        let mut response = $response;

        response = response.with_body_reader($body);

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) transfer_encoding: $value:expr, $($tail:tt)*) => {{
        let mut response = $response;

        if $value {
            response.body_len = None;
        }

        $crate::mock!(@response(response) $($tail)*)
    }};

    (@response($response:expr) delay: $delay:tt, $($tail:tt)*) => {{
        let duration = $crate::macro_api::parse_duration(stringify!($delay));
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

        impl<F> Responder<F>
        where
            F: Send + Sync + 'static + for<'r> Fn(&'r $crate::Request) -> Option<$crate::Response>,
        {
            fn new(f: F) -> Self {
                Self(f)
            }
        }

        impl<F> $crate::Responder for Responder<F>
        where
            F: Send + Sync + 'static + for<'r> Fn(&'r $crate::Request) -> Option<$crate::Response>,
        {
            fn respond(&self, ctx: &mut $crate::RequestContext<'_>) {
                if let Some(response) = (self.0)(ctx.request()) {
                    ctx.send(response);
                }
            }
        }

        $crate::Mock::new(Responder::new(move |request| {
            let mut response = $crate::Response::default();

            let response = $crate::mock!(@response(response) $($inner)*);

            Some(response)
        }))
    }};
}
