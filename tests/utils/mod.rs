macro_rules! assert_matches {
    ($value:expr, $($pattern:tt)+) => {{
        match $value {
            $($pattern)* => {},
            value => panic!(
                "assertion failed: `{}` matches `{}`\n  value: `{:?}`",
                stringify!($value),
                stringify!($($pattern)*),
                value,
            ),
        }
    }};
}
