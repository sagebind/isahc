# Contributing Guide

Thanks for considering

## Code contributions

If you'd like to get a deeper understanding of how Isahc is structured internally, check out the [Isahc Internals] document.

### Formatting

Isahc code largely follows the [Rust Style Guide], with a few minor tweaks to improve readability on otherwise dense lines. These tweaks are configured in [`rustfmt.toml`](rustfmt.toml). Since nightly-only options are used, you must use a nightly version of rustfmt in order to run auto-formatting. If you are using Rust with Rustup, this is simply running `cargo +nightly fmt`.


[Isahc Internals]: INTERNALS.md
[Rust Style Guide]: https://github.com/rust-dev-tools/fmt-rfcs/blob/master/guide/guide.md
