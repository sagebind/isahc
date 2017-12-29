# cHTTP
The practical HTTP client that is fun to use.

[![Shippable](https://img.shields.io/shippable/59f4fe44571f17060086ff61.svg)](https://app.shippable.com/github/sagebind/chttp)
[![Crates.io](https://img.shields.io/crates/v/chttp.svg)](https://crates.io/crates/chttp)
[![Documentation](https://docs.rs/chttp/badge.svg)](https://docs.rs/chttp)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

[Documentation](https://docs.rs/chttp)

cHTTP provides a clean and easy-to-use interface around the venerable [libcurl]. Here are some of the features that are currently available:

- HTTP/2 support (if libcurl is compiled with it).
- Connection pooling and reuse.
- Respone body streaming.
- Request body uploading from memory or a stream.
- Tweakable redirect policy.
- TCP socket configuration.
- Uses the future standard Rust [http] interface for requests and responses.

## Why [libcurl]?
Not everything needs to be re-invented! For typical use cases, [libcurl] is a fantastic choice for making web requests. It's fast, reliable, well supported, and isn't going away any time soon.

It has a reputation for having an unusual API that is sometimes tricky to use, but hey, that's why this library exists.

## Examples
Really simple example that spits out the response body from https://example.org:

```rust
let mut response = chttp::get("https://example.org").unwrap();
let body = response.body_mut().text().unwrap();
println!("{}", body);
```

Configuring a custom client:

```rust
use chttp::{http, Client, Options, RedirectPolicy};
use std::time::Duration;

let mut options = Options::default();
options.timeout = Some(Duration::from_secs(60));
options.redirect_policy = RedirectPolicy::Limit(10);
options.preferred_http_version = Some(http::Version::HTTP_2);

let client = Client::builder()
    .max_connections(Some(4))
    .options(options)
    .build();

let mut response = client.get("https://example.org").unwrap();
let body = response.body_mut().text().unwrap();
println!("{}", body);
```

## Requirements
On Linux:

- libcurl 7.24.0 or newer
- OpenSSL 1.0.1, 1.0.2, 1.1.0, or LibreSSL

On Windows and macOS:

- TBD

## Installation
Add this to your Cargo.toml file:

```toml
[dependencies]
chttp = "0.1"
```

## License
This library is licensed under the MIT license. See the [LICENSE](LICENSE) file for details.


[http]: https://github.com/hyperium/http
[libcurl]: https://curl.haxx.se/libcurl/
