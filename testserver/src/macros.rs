/// Macro to define a mock endpoint using a more concise DSL.
#[macro_export]
macro_rules! mock {
    () => {
        $crate::mock! {
            _ => {},
        }
    };

    ($($inner:tt)*) => {{
        let mut builder = $crate::Mock::builder();

        $crate::__mock_impl!(@responders(builder) $($inner)*);

        builder.build()
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __mock_impl {
    (
        @responders($builder:ident)
        /$($path:tt)? => $response:tt,
        $($tail:tt)*
    ) => {
        $builder = $builder.responder($crate::macro_api::ClosureResponder::new(move |ctx| {
            if ctx.request().url() == stringify!(/$($path)*) {
                $crate::__mock_impl!(@responder(ctx) $response);
            }
        }));

        $crate::__mock_impl!(@responders($builder) $($tail)*);
    };

    (
        @responders($builder:ident)
        #$num:expr => writer |$writer:ident| {
            $($body:tt)*
        },
        $($tail:tt)*
    ) => {
        $builder = $builder.responder($crate::macro_api::ClosureResponder::new(move |ctx| {
            if ctx.request().number() == $num {
                let mut $writer = ctx.into_raw();
                {
                    $($body)*
                }
                let _ = $writer.flush();
            }
        }));

        $crate::__mock_impl!(@responders($builder) $($tail)*);
    };

    (
        @responders($builder:ident)
        #$num:expr => {
            $($response_attrs:tt)*
        },
        $($tail:tt)*
    ) => {
        $builder = $builder.responder($crate::macro_api::ClosureResponder::new(move |ctx| {
            if ctx.request().number() == $num {
                let mut response = $crate::Response::default();

                $crate::__mock_impl!(@response(response) $($response_attrs)*);

                ctx.send(response);
            }
        }));

        $crate::__mock_impl!(@responders($builder) $($tail)*);
    };

    (
        @responders($builder:ident)
        _ => writer |$writer:ident| {
            $($body:tt)*
        },
        $($tail:tt)*
    ) => {
        $builder = $builder.responder($crate::macro_api::ClosureResponder::new(move |ctx| {
            let mut $writer = ctx.into_raw();
            {
                $($body)*
            }
            let _ = $writer.flush();
        }));

        $crate::__mock_impl!(@responders($builder) $($tail)*);
    };

    (
        @responders($builder:ident)
        _ => {
            $($response_attrs:tt)*
        },
        $($tail:tt)*
    ) => {
        $builder = $builder.responder($crate::macro_api::ClosureResponder::new(move |ctx| {
            let mut response = $crate::Response::default();

            $crate::__mock_impl!(@response(response) $($response_attrs)*);

            ctx.send(response);
        }));

        $crate::__mock_impl!(@responders($builder) $($tail)*);
    };

    // For backwards compatibility.
    (@responders($builder:ident) $($response_attrs:tt)+) => {
        $builder = $builder.responder($crate::macro_api::ClosureResponder::new(move |ctx| {
            let mut response = $crate::Response::default();

            $crate::__mock_impl!(@response(response) $($response_attrs)*);

            ctx.send(response);
        }));
    };

    (@responders($builder:ident)) => {};

    (@response($response:ident) status: $status:expr, $($tail:tt)*) => {
        $response.status_code = $status as u16;

        $crate::__mock_impl!(@response($response) $($tail)*)
    };

    (@response($response:ident) body: $body:expr, $($tail:tt)*) => {
        $response = $response.with_body_buf($body);

        $crate::__mock_impl!(@response($response) $($tail)*)
    };

    (@response($response:ident) body_reader: $body:expr, $($tail:tt)*) => {
        $response = $response.with_body_reader($body);

        $crate::__mock_impl!(@response($response) $($tail)*)
    };

    (@response($response:ident) transfer_encoding: $value:expr, $($tail:tt)*) => {
        if $value {
            $response.body_len = None;
        }

        $crate::__mock_impl!(@response($response) $($tail)*)
    };

    (@response($response:ident) delay: $delay:tt, $($tail:tt)*) => {
        let duration = $crate::macro_api::parse_duration(stringify!($delay));
        ::std::thread::sleep(duration);

        $crate::__mock_impl!(@response($response) $($tail)*)
    };

    (@response($response:ident) headers {
        $(
            $name:literal: $value:expr,
        )*
    } $($tail:tt)*) => {
        $(
            $response.headers.push(($name.to_string(), $value.to_string()));
        )*

        $crate::__mock_impl!(@response($response) $($tail)*)
    };

    (@response($response:ident)) => {};
}

#[doc(hidden)]
pub mod macro_api {
    use std::time::Duration;

    pub fn parse_duration(s: &str) -> Duration {
        humantime::parse_duration(s).unwrap()
    }

    pub struct ClosureResponder<F>(F);

    impl<F> ClosureResponder<F>
    where
        F: Send + Sync + 'static + for<'r> Fn(&'r mut crate::RequestContext<'_>),
    {
        pub fn new(f: F) -> Self {
            Self(f)
        }
    }

    impl<F> crate::Responder for ClosureResponder<F>
    where
        F: Send + Sync + 'static + for<'r> Fn(&'r mut crate::RequestContext<'_>),
    {
        fn respond(&self, ctx: &mut crate::RequestContext<'_>) {
            (self.0)(ctx)
        }
    }
}
