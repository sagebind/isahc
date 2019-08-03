# Isahc (formerly cHTTP)

Say hello to Isahc (pronounced like _Isaac_), the practical HTTP client that is fun to use.

[![Crates.io](https://img.shields.io/crates/v/isahc.svg)](https://crates.io/crates/isahc)
[![Documentation](https://docs.rs/isahc/badge.svg)][documentation]
![License](https://img.shields.io/badge/license-MIT-blue.svg)

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

Safe Rust bindings to libcurl are provided by the [curl](https://crates.io/crates/curl) crate, which you can use yourself if you want to use curl directly. Isahc delivers a lot of value on top of vanilla curl, by offering a simpler, more idiomatic API and doing the hard work of turning the powerful [multi interface] into a futures-based API.

## Installation

Install via Cargo by adding to your `Cargo.toml` file:

```toml
[dependencies]
isahc = "0.6"
```

### Supported Rust versions

The current release is only guaranteed to work with the latest stable Rust compiler. When Isahc reaches version `1.0`, a more conservative policy will be adopted.

### Feature flags

Isahc is designed to be as "pay-as-you-need" as possible using Cargo feature
flags and optional dependencies. Unstable features are also initially
released behind feature flags until they are stabilized. You can add the
feature names below to your `Cargo.toml` file to enable them:

```toml
[dependencies.isahc]
version = "0.6"
features = ["psl"]
```

Below is a list of all available feature flags and their meanings.

- `cookies`: Enable persistent HTTP cookie support. Enabled by default.
- `http2`: Enable HTTP/2 support in libcurl via libnghttp2. Enabled by default.
- `json`: Additional serialization and deserialization of JSON bodies via [serde]. Disabled by default.
- `psl`: Enable use of the Public Suffix List to filter out potentially malicious cross-domain cookies. Disabled by default.
- `static-curl`: Use a bundled libcurl version and statically link to it. Enabled by default.
- `middleware-api`: Enable the new middleware API. Unstable until the API is finalized. This an unstable feature whose interface may change between patch releases.

## [Documentation]

Please check out the [documentation] for details on what Isahc can do and how to use it.

To get you started, here is a really simple example that spits out the response body from https://example.org:

```rust
// Send a GET request and wait for the response.
let mut response = isahc::get("https://example.org")?
// Read the response body into a string and print it to standard output.
let body = response.body_mut().text()?;
println!("{}", body);
```

## License

This project's source code and documentation is licensed under the MIT license. See the [LICENSE](LICENSE) file for details.

The "Isahc" logo and mascot may only be used as required for reasonable and customary use in describing the Isahc project and in redistribution of the project.


[async/await]: https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html
[documentation]: https://docs.rs/isahc
[http]: https://github.com/hyperium/http
[libcurl]: https://curl.haxx.se/libcurl/
[multi interface]: https://curl.haxx.se/libcurl/c/libcurl-multi.html
[rust]: https://www.rustlang.org
[serde]: https://serde.rs
