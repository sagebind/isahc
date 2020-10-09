# Isahc

Say hello to Isahc (pronounced like _Isaac_), the practical HTTP client that is fun to use.

_Formerly known as [chttp]._

[![Crates.io](https://img.shields.io/crates/v/isahc.svg)](https://crates.io/crates/isahc)
[![Documentation](https://docs.rs/isahc/badge.svg)][documentation]
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Crates.io downloads](https://img.shields.io/crates/d/isahc)](https://crates.io/crates/isahc)
![Maintenance](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg)
[![Build](https://github.com/sagebind/isahc/workflows/ci/badge.svg)](https://github.com/sagebind/isahc/actions)
[![codecov](https://codecov.io/gh/sagebind/isahc/branch/master/graph/badge.svg)](https://codecov.io/gh/sagebind/isahc)

## Key features

- Full support for HTTP/1.1 and HTTP/2.
- Configurable request timeouts.
- Fully asynchronous core, with asynchronous and incremental reading and writing of request and response bodies.
- Offers an ergonomic synchronous API as well as an asynchronous API with support for [async/await].
- Optional automatic redirect following.
- Sessions and cookie persistence.
- Request cancellation on drop.
- Tweakable redirect policy.
- Network socket configuration.
- Uses the [http] crate as an interface for requests and responses.

<img src="media/isahc.svg.png" width="320" align="right">

## What is Isahc?

Isahc is an acronym that stands for **I**ncredible **S**treaming **A**synchronous **H**TTP **C**lient, and as the name implies, is an asynchronous HTTP client for the [Rust] language. It uses [libcurl] as an HTTP engine inside, and provides an easy-to-use API on top that integrates with Rust idioms.

## No, _who_ is Isahc?

Oh, you mean Isahc the dog! He's an adorable little Siberian husky who loves to play fetch with webservers every day and has a very _cURLy_ tail. He shares a name with the project and acts as the project's mascot.

You can pet him all day if you like, he doesn't mind. Though, he prefers it if you pet him in a standards-compliant way!

## Why use Isahc and not X?

Isahc provides an easy-to-use, flexible, and idiomatic Rust API that makes sending HTTP requests a breeze. The goal of Isahc is to make the easy way _also_ provide excellent performance and correctness for common use cases.

Isahc uses [libcurl] under the hood to handle the HTTP protocol and networking. Using curl as an engine for an HTTP client is a great choice for a few reasons:

- It is a stable, actively developed, and very popular library.
- It is well-supported on a diverse list of platforms.
- The HTTP protocol has a lot of unexpected gotchas across different servers, and curl has been around the block long enough to handle many of them.
- It is well optimized and offers the ability to implement asynchronous requests.

Safe Rust bindings to libcurl are provided by the [curl crate], which you can use yourself if you want to use curl directly. Isahc delivers a lot of value on top of vanilla curl, by offering a simpler, more idiomatic API and doing the hard work of turning the powerful [multi interface] into a futures-based API.

## [Documentation]

Please check out the [documentation] for details on what Isahc can do and how to use it. To get you started, here is a really simple, complete example that spits out the response body from https://example.org:

```rust
use isahc::prelude::*;

fn main() -> Result<(), isahc::Error> {
    // Send a GET request and wait for the response headers.
    // Must be `mut` so we can read the response body.
    let mut response = isahc::get("http://example.org")?;

    // Print some basic info about the response to standard output.
    println!("Status: {}", response.status());
    println!("Headers: {:#?}", response.headers());

    // Read the response body as text into a string and print it.
    print!("{}", response.text()?);

    Ok(())
}
```

Click [here][documentation] for documentation on the latest version. You can also click [here](https://sagebind.github.io/isahc/isahc/) for built documentation from the latest unreleased `master` build.

## Installation

Install via Cargo by adding to your `Cargo.toml` file:

```toml
[dependencies]
isahc = "0.9"
```

### Supported Rust versions

The current release is only guaranteed to work with the latest stable Rust compiler. When Isahc reaches version `1.0`, a more conservative policy will be adopted.

## Project goals

- Create an ergonomic and innovative HTTP client API that is easy for beginners to use, and flexible for advanced uses.
- Provide a high-level wrapper around libcurl.
- Maintain a lightweight dependency tree and small binary footprint.
- Provide additional client features that may be optionally compiled in.

Non-goals:

- Support for protocols other than HTTP.
- Alternative engines besides libcurl. Other projects are better suited for this.

## When would you *not* use Isahc?

Not every library is perfect for every use-case. While Isahc strives to be a full-featured and general-purpose HTTP client that should work well for many projects, there are a few scenarios that Isahc is not well suited for:

- **Tiny binaries**: If you are creating an application where tiny binary size is a key priority, you might find Isahc to be too large for you. While Isahc's dependencies are carefully curated and a number of features can be disabled, Isahc's core feature set includes things like async which does have some file size overhead. You might find something like [ureq] more suitable.
- **WebAssembly support**: If your project needs to be able to be compiled to WebAssembly, then Isahc will probably not work for you. Instead you might like an HTTP client that supports multiple backends such as [Surf].
- **Rustls support**: We hope to support [rustls] as a TLS backend someday, it is not currently supported directly. If for some reason rustls is a hard requirement for you, you'll need to use a different HTTP client for now.

## License

This project's source code and documentation are licensed under the MIT license. See the [LICENSE](LICENSE) file for details.

The Isahc logo and related assets are licensed under a [Creative Commons Attribution 4.0 International License][cc-by]. See [LICENSE-CC-BY](LICENSE-CC-BY) for details.


[async/await]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
[cc-by]: http://creativecommons.org/licenses/by/4.0/
[chttp]: https://crates.io/crates/chttp
[curl crate]: https://crates.io/crates/curl
[documentation]: https://docs.rs/isahc
[http]: https://github.com/hyperium/http
[libcurl]: https://curl.haxx.se/libcurl/
[MIT Kerberos]: https://web.mit.edu/kerberos/
[multi interface]: https://curl.haxx.se/libcurl/c/libcurl-multi.html
[rfc4559]: https://tools.ietf.org/html/rfc4559
[rust]: https://www.rustlang.org
[rustls]: https://github.com/ctz/rustls
[serde]: https://serde.rs
[Surf]: https://github.com/http-rs/surf
[ureq]: https://github.com/algesten/ureq
