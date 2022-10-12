//! Logging facade that delegates to either the `log` crate or the `tracing`
//! crate depending on compile-time features.

#[cfg(feature = "tracing")]
pub(crate) type Span = tracing::Span;

#[cfg(not(feature = "tracing"))]
#[derive(Clone)]
pub(crate) struct Span;

macro_rules! span {
    ($level:ident, $($t:tt)+) => {{
        #[cfg(feature = "tracing")]
        {
            ::tracing::span!(::tracing::Level::$level, $($t)*)
        }

        #[cfg(not(feature = "tracing"))]
        {
            $crate::log::Span
        }
    }};
}

macro_rules! span_follows_from_current {
    ($span:expr) => {{
        #[cfg(feature = "tracing")]
        $span.follows_from(::tracing::Span::current());
    }};
}

macro_rules! enter_span {
    ($span:expr) => {{
        #[cfg(feature = "tracing")]
        let _enter = $span.enter();

        #[cfg(not(feature = "tracing"))]
        let _enter = &$span;
    }};
}

macro_rules! instrument_span {
    ($span:expr, $future:expr) => {{
        #[cfg(feature = "tracing-futures")]
        {
            ::tracing_futures::Instrument::instrument($future, $span)
        }

        #[cfg(not(feature = "tracing-futures"))]
        {
            let _span = $span;
            $future
        }
    }};
}

macro_rules! error {
    ($($t:tt)+) => {{
        #[cfg(feature = "tracing")]
        ::tracing::error!($($t)*);

        #[cfg(not(feature = "tracing"))]
        ::log::error!($($t)*);
    }};
}

macro_rules! warn {
    ($($t:tt)+) => {{
        #[cfg(feature = "tracing")]
        ::tracing::warn!($($t)*);

        #[cfg(not(feature = "tracing"))]
        ::log::warn!($($t)*);
    }};
}

macro_rules! info {
    ($($t:tt)+) => {{
        #[cfg(feature = "tracing")]
        ::tracing::info!($($t)*);

        #[cfg(not(feature = "tracing"))]
        ::log::info!($($t)*);
    }};
}

macro_rules! debug {
    ($($t:tt)+) => {{
        #[cfg(feature = "tracing")]
        ::tracing::debug!($($t)*);

        #[cfg(not(feature = "tracing"))]
        ::log::debug!($($t)*);
    }};
}

macro_rules! trace {
    ($($t:tt)+) => {{
        #[cfg(feature = "tracing")]
        ::tracing::trace!($($t)*);

        #[cfg(not(feature = "tracing"))]
        ::log::trace!($($t)*);
    }};
}
