# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.0](https://github.com/sagebind/isahc/compare/isahc-v1.8.1...isahc-v2.0.0) - 2026-04-21

### Feat

- *(net)* [**breaking**] Support multiple selectors for interface configuration ([#494](https://github.com/sagebind/isahc/pull/494))

## [1.8.1](https://github.com/sagebind/isahc/compare/isahc-v1.8.0...isahc-v1.8.1) - 2026-04-11

### Fixed

- Fix an agent panic in selector that would sometimes be triggered by multiple socket interests communicated from libcurl for the same socket. Error handling for the agent thread has also been improved in general. ([#460](https://github.com/sagebind/isahc/issues/460), [#481](https://github.com/sagebind/isahc/pull/481))
- Fix a segfault that can occur when the agent thread panics and drops a curl multi handle before its easy handles. This is a bug in the curl-rust project, but we have implemented a workaround that should prevent this from happening for any version of curl-rust you might be using. ([#459](https://github.com/sagebind/isahc/issues/459), [#461](https://github.com/sagebind/isahc/pull/461), [#480](https://github.com/sagebind/isahc/pull/480))
- Rewrite PSL cache test to not use globals so that they don't randomly fail in CI. ([#485](https://github.com/sagebind/isahc/pull/485))
- Improve `Debug` impl for empty request bodies to distinguish between an empty body and no body.
- Fix docs.rs failing builds.

### Dependency Updates

- Remove parking lot dependency.
- Bump psl from 2.1.200 to 2.1.202 (#484, #486)

## [1.8.0] - 2026-03-31

This is the first maintenance release in a few years and includes a few housekeeping items. No major changes or additions.

### Changed

- Isahc's MSRV has been increased from 1.46.0 to 1.85.0. (#446)
- The [Public Suffix List](https://publicsuffix.org) Git submodule used for compile-time suffix checking has been removed in favor of the [`psl` crate](https://github.com/addr-rs/psl) which provides the same functionality. (#477)
- Releases are now managed by [release-plz](https://release-plz.dev) instead of custom workflows.

### Dependency Updates

- Many dependencies have been updated to their latest versions, which should help with compilation in modern environments. Some dependencies now allow for multiple major versions.
- `once_cell` has been removed in favor of the equivalent types now provided in `std`. (#478)

## [1.7.2] - 2022-05-13

### Security

- Upgrade `curl-sys` to 0.4.55 to pull in [libcurl 7.83.1](https://curl.se/changes.html#7_83_1), which contains security patches for the below vulnerabilities. (#394) @sagebind
    - [CVE-2022-27778: curl removes wrong file on error](https://curl.se/docs/CVE-2022-27778.html)
    - [CVE-2022-27779: cookie for trailing dot TLD](https://curl.se/docs/CVE-2022-27779.html)
    - [CVE-2022-27780: percent-encoded path separator in URL host](https://curl.se/docs/CVE-2022-27780.html)
    - [CVE-2022-27781: CERTINFO never-ending busy-loop](https://curl.se/docs/CVE-2022-27781.html)
    - [CVE-2022-27782: TLS and SSH connection too eager reuse](https://curl.se/docs/CVE-2022-27782.html)
    - [CVE-2022-30115: HSTS bypass via trailing dot](https://curl.se/docs/CVE-2022-30115.html)
- Fix several bugs with the `auto_referer` option (disabled by default) which could potentially result in sensitive headers being passed to redirect targets unintentionally. (#393) @sagebind
    - Fix multiple `Referer` headers being included when two or more redirects are followed in a request
    - URL fragments and userinfo parts of the URL authority should not be included in the `Referer` header
    - Don't include a `Referer` header when redirecting from an HTTPS URL to an HTTP URL, as per [RFC 7231](https://httpwg.org/specs/rfc7231.html#header.referer) recommendation
    - Scrub sensitive headers when redirecting to a different authority

### Dependency Updates

- Update Public Suffix List to 172bbfd (#395) @teto-bot

## [1.7.1] - 2022-04-28

### Security

- Update `curl-sys` to 0.4.54 to pull in [libcurl 7.83.0](https://curl.se/changes.html#7_83_0), which contains security patches for [CVE-2022-22576](https://curl.se/docs/CVE-2022-22576.html), [CVE-2022-27774](https://curl.se/docs/CVE-2022-27774.html), [CVE-2022-27775](https://curl.se/docs/CVE-2022-27775.html), and [CVE-2022-27776](https://curl.se/docs/CVE-2022-27776.html). (#391) @david-perez

## [1.7.0] - 2022-03-12

### Added

- Add new [`is_http_version_supported`](https://docs.rs/isahc/latest/isahc/fn.is_http_version_supported.html) function which allows you to check whether support for a particular HTTP version is available at runtime. When statically linking this will be entirely dependent on your build configuration, but if you are dynamically linking to libcurl then it will vary from system to system. (#368) @sagebind

### Changed

- Preallocate buffer for async JSON decoding to improve performance. (#367) @michalmuskala
- Re-enable content-length request test (#383) @sagebind
- Add minimal versions test to CI (#373) @sagebind
- Refactor test server to support writing raw response data. (#366) @sagebind

### Dependency Updates

- Update test-case requirement from 1.1 to 2.0 (#376) @dependabot

## [1.6.0] - 2021-11-13

### Added

- Expose new APIs for cookie construction, updating, and adding to cookie jar. You can now create your own cookies with `Cookie::builder` and put arbitrary cookies into the cookie jar with `CookieJar::set`. (#264, #349) @jacobmischka
- Add `bytes()` convenience methods to `ReadResponseExt` and `AsyncReadResponseExt` which read the entire response body into a `Vec<u8>`. (#352) @sagebind
- Speed up CI by adding caching to CI (#358) @sagebind

### Security

- Replace trivial internal usage of chrono with httpdate to avoid any potential reference to [CVE-2020-26235 in `time` 0.2](https://github.com/rustsec/advisory-db/blob/9e93a3df4a54e70f2539a2ecdc3d70beef64c856/crates/time/RUSTSEC-2020-0071.md). (#361) @sagebind

### Dependency Updates

- Replace chrono with httpdate internally (#361) @sagebind
- Update tracing-subscriber requirement from 0.2.12 to 0.3.0 (#360) @dependabot
- Update Tarpaulin and re-enable doctest coverage (#357) @sagebind
- Update tiny_http requirement from 0.8 to 0.9 (#356) @dependabot

## [1.5.1] - 2021-10-13

### Fixed

- Greatly reduce CPU usage, particularly when receiving long-running or large responses. This was caused by a bug where timeout timers were not being cleared once they expired, effectively creating a repeating timer that would cause repeated extra polls. **Huge** thanks to @jacobmischka for finding and fixing this bug! (#348, #350)
- Return true for `Body::is_empty` for `HEAD` responses (#341, #343)
- Fix code coverage analysis failing to run in CI. (#351)

## [1.5.0] - 2021-08-23

### Added

- Expose connection info in errors with the addition of `Error::local_addr` and `Error::remote_addr`. This allows you to get the local & remote addresses involved in a request, if any, even if an error occurs. (#336, #337) @sagebind
- Allow use of the `Expect` header to be configured via `Configurable::expect_continue`. (#303, #311, #340) @sagebind

### Dependency Updates

- Update env_logger requirement from 0.8 to 0.9 (#330) @dependabot

## [1.4.1] - 2021-08-18

### Added

- Improve the documentation on `Error` and `ErrorKind` and add `Error::is_timeout`.

### Fixed

- Improve connection reuse log message and fix some false positives around its emission. The warning about connection reuse will now point users to [the wiki page](https://github.com/sagebind/isahc/wiki/Connection-Reuse) which explains the message in depth. (#335) @sagebind

### Dependency Updates

- Update Public Suffix List to bc5d64d (#328) @teto-bot

## [1.4.0] - 2021-05-14

### Added

- Add support for using in-memory client certificates. (#89, #320)
- Add API for accessing trailer headers in responses. (#157, #256)

### Changed

- Isahc's MSRV has been increased from 1.41.0 to 1.46.0. (#321)

### Fixed

- Fix wrong link to request type in docs. (#323) @humb1t

### Dependency Updates

- Update Public Suffix List to 598c638 (#325) @teto-bot

## [1.3.1] - 2021-04-16

### Dependency Updates

- Update Public Suffix List to 5cb7ed8 (#319) @teto-bot
- Update curl version constraint to ensure MSRV.

## [1.3.0] - 2021-04-07

### Added

- Allow configuring low speed timeouts for transfers, either per-request or as a client default. (#316) @MoSal

### Fixed

- Handle raw UTF-8 bytes in redirect headers (#315, #317) @sagebind
- Fix agent IDs for tracing always `0`. @sagebind

### Changed

- Replace flume with async-channel internally to maintain MSRV contract. (#318) @sagebind

## [1.2.0] - 2021-03-23

This release contains some minor performance improvements as a result of some internal changes.

### Changed

- Switch agent from curl-provided `select(2)` backend to [`polling`](https://github.com/stjepang/polling). This delivers some throughput improvements in some benchmarks involving concurrent requests. This also removes Isahc's reliance on loopback UDP sockets for selector wakeups. (#17, #243, #263) @sagebind
- Refactor request configuration internal representation. This offered a minor performance improvement in some cases by greatly reducing the amount of hashmap lookups needed for applying request configuration. (#292) @sagebind

### Dependency Updates

- Upgrade publicsuffix to v2 (#312) @rushmorem
- Update tiny_http requirement from 0.7 to 0.8 (#296) @dependabot

## [1.1.0] - 2021-01-30

### Added

- Add async `json()` response convenience method to deserialize JSON asynchronous responses to mirror the synchronous one. (#245, #291)
- Add consume API for reading response bodies fully before discarding them. (#257, #284)

### Fixed

- Update sluice to pull in race condition bugfix. (#295)

## [1.0.3] - 2021-01-11

### Fixed

- Fix parsing of quoted cookie values (#288) @theawless

### Dependency Updates

- Update Public Suffix List to 6b67c6f (#289) @teto-bot

## [1.0.2] - 2021-01-01

### Fixed

- Headers for HTTP/1.x are now always sent with a single trailing space after the colon (`:`). While not strictly necessary according to RFC 7230, it was uncommon formatting and poorly-written servers can choke on parsing such headers. (#286, #287)

## [1.0.1] - 2020-12-31

### Fixed

- Update future type returned by `AsyncReadResponseExt::copy_to` to implement `Send` if both the reader and writer types implement `Send`. This allows it to work with multithreaded runtimes. (#283, #285)

## [1.0.0] - 2020-12-29

### Breaking Changes

- The `Body` type has now been broken up into distinct `AsyncBody` and `Body` types, with the former implementing only `AsyncRead` and the latter implementing only `Read`. This was done to reduce confusion on how to produce and consume body content when in an asynchronous context without blocking. This also makes it possible to use synchronous `Read` sources such as a `File` as a request body when using the synchronous API, something that was previously difficult to do. (#202, #262)
- Methods on the `ResponseExt` trait related to reading the response body have been extracted into two new extension traits: `AsyncReadResponseExt` and `ReadResponseExt`. Like the previous change, this was done to reduce confusion on which methods to use when consuming a response in an async context. The `_async` suffix previously used to distinguish between the sync and async methods has been dropped, as it is no longer necessary. (#202, #262)
- The `Error` type has been significantly refactored and changed into a struct with a separate `ErrorKind` enum. This was done to make it possible to add new errors without breaking changes, and to ensure that errors can always preserve upstream causes efficiently. The error kinds have also been updated to be clearer and more distinct. (#182, #258)
- The `bytes` crate is no longer a dependency and `Body::from_maybe_shared` has been removed. (#261)
- `Configurable::dns_servers` has been removed, as it is more likely to confuse users more than anything since it requires libcurl to be compiled with c-ares, which it isn't by default and is unlikely to be.
- Removed `Request`, `Response`, and `HttpClient` from the `prelude` module. You will now have to import these directly. Importing large prelude modules can make code more confusing to read and is usually considered an anti-pattern. (#281)

### Fixed

- Fix warning for aborting response body stream early being emitted inconsistently. Also change from a `WARN` to an `INFO` log. (#280)

### Other Changes

- The minimum supported Rust version (MSRV) is now pinned to 1.41. (#259)
- Add `post_async` example usage and improve various method docs. (#273)
- Add rustfmt config and apply rustfmt to the entire codebase. (#276, #277)
- Add Clippy checks to CI. (#279)

### Dependency Updates

- Update Public Suffix List to f9f612a (#266)
- Update flume requirement from 0.9 to 0.10 (#271)

## [1.0.0-beta.1] - 2020-12-09

### Breaking Changes

- The `Body` type has now been broken up into distinct `AsyncBody` and `Body` types, with the former implementing only `AsyncRead` and the latter implementing only `Read`. This was done to reduce confusion on how to produce and consume body content when in an asynchronous context without blocking. This also makes it possible to use synchronous `Read` sources such as a `File` as a request body when using the synchronous API, something that was previously difficult to do. (#202, #262)
- Methods on the `ResponseExt` trait related to reading the response body have been extracted into two new extension traits: `AsyncReadResponseExt` and `ReadResponseExt`. Like the previous change, this was done to reduce confusion on which methods to use when consuming a response in an async context. The `_async` suffix previously used to distinguish between the sync and async methods has been dropped, as it is no longer necessary. (#202, #262)
- The `Error` type has been significantly refactored and changed into a struct with a separate `ErrorKind` enum. This was done to make it possible to add new errors without breaking changes, and to ensure that errors can always preserve upstream causes efficiently. The error kinds have also been updated to be clearer and more distinct. (#182, #258)
- The `bytes` crate is no longer a dependency and `Body::from_maybe_shared` has been removed. (#261)
- `Configurable::dns_servers` has been removed, as it is more likely to confuse users more than anything since it requires libcurl to be compiled with c-ares, which it isn't by default and is unlikely to be.

### Other Changes

- The minimum supported Rust version (MSRV) is now pinned to 1.41. (#259)

### Dependency Updates

- Update Public Suffix List to f9f612a (#266) @sagebind

## [0.9.14] - 2020-12-09

### Fixed

- Fix body length incorrectly returning the length of the compressed body when the server combines compression and `Content-Length` with auto decompression enabled. (#265, #267)

## [0.9.13] - 2020-11-14

### Added

- Add `Configurable::ip_version` which allows you to restrict resolving hostnames to a specific IP version. (#252, #253) @ArenM

### Fixed

- Fix redirect handling with redirect responses that include non-empty bodies, another regression introduced in 0.9.11. (#250, #255) @sagebind

### Changed

- Trim some heavy dependencies. (#254) @sagebind

## [0.9.12] - 2020-11-11

### Fixed

- Fix a regression introduced in 0.9.11 resulting in client-wide redirect policies not being respected. (#250, #251) @sagebind

### Changed

- Improve documentation on timeouts. (#249) @sagebind

### Dependency Updates

- Update Public Suffix List to 6400969 (#248) @sagebind

## [0.9.11] - 2020-11-03

A surprisingly feature-focused patch release with a couple notable bugfixes. This October Isahc opted-in to [Hacktoberfest](https://hacktoberfest.digitalocean.com), and we received a couple additions from new contributors. Thanks!

### Added

- Add `HttpClientBuilder::connection_cache_ttl` for configuring how long to keep connections open in the cache. (#93, #237) @gsquire
- Make cookie jar API more useful by adding several new methods, including `HttpClient::cookie_jar`, `Configurable::cookie_jar`, `CookieJar::get_by_name`, `Cookie::value`, and more! An example of how to use some of these have been added to `examples/cookies.rs`. (#215, #233) @sagebind
- Add a "Why not use" section to readme. (#234) @sagebind

### Fixed

- Fix timeouts and other mid-transfer errors causing unexplained EOFs instead of returning a proper `io::Error`. (#154, #246) @sagebind
- Fix improper cookie behavior when automatically following redirects, such as not sending any cookies in subsequent requests. (#232, #240) @sagebind

### Changed

- Make `HttpClient` cloneable. This makes it much more convenient to share a client instance between threads or tasks. (#241, #244) @braunse
- Replace middleware API with interceptors API. The `middleware-preview` crate feature has been removed and the `unstable-interceptors` feature has been added. The API is still unstable, but addresses a number of problems with the old proposed middleware API. (#42, #206) @sagebind
- Update integration tests to use new testserver (#230) @sagebind

### Dependency Updates

- Update env_logger requirement from 0.7 to 0.8 (#238) @dependabot
- Update crossbeam-channel requirement from 0.4 to 0.5 (#236) @dependabot
- Update crossbeam-utils requirement from 0.7 to 0.8 (#235) @dependabot
- Update Public Suffix List to 40d5bd4 (#231) @sagebind
- Create Dependabot config file (#229) @dependabot-preview

## [0.9.10] - 2020-10-01

### Added

- Add `automatic_decompression` option to allow you to disable the automatic response body decompression or enable it on a per-request basis. (#227, #228)

## [0.9.9] - 2020-09-23

### Added

- Add static-ssl feature to mirror curl/static-ssl (#225) @SecurityInsanity

### Dependency Updates

- Update Public Suffix List to 5b2327d (#224) @sagebind

## [0.9.8] - 2020-08-08

### Added

- Add title_case_headers option (#205, #218) @sagebind
- Add local_addr and remote_addr getters (#220, #221) @sagebind

### Dependency Updates

- Update Public Suffix List to 54eae6e (#222) @sagebind

## [0.9.7] - 2020-07-29

### Added

- Add new `config::Dialer` API that allows you to customize and override what sockets are connected to for a request, regardless of the host in the URL. Static IP sockets and Unix sockets are initially supported. (#150, #207) @sagebind

### Fixed

- Fix HEAD requests with a body and libcurl 7.71+ not playing well together because of incorrect usage of `CURLOPT_NOBODY` in Isahc. (#213, #214, #216) @sagebind

### Dependency Updates

- Update mockito requirement from 0.26 to 0.27 (#217) @dependabot-preview

## [0.9.6] - 2020-07-21

### Fixed

- Fix empty and blank request header values not being sent. (#209, #210)

### Dependency Updates

- Update Public Suffix List to 011f110 (#204)
- Update mockito requirement from 0.25 to 0.26 (#203) @dependabot-preview
- Update parking_lot requirement from 0.10 to 0.11 (#201) @dependabot-preview

## [0.9.5] - 2020-06-24

### Fixed

- Upgrade curl to 0.4.30 to mitigate potential init-on-non-main-thread with certain TLS engines on certain platforms. (#189) @sagebind

### Added

- Allow for experimental HTTP/3 support in libcurl. (This does not enable HTTP/3 support, it just merely allows it if libcurl is compiled with it.) (#185) @sagebind

### Dependency Updates

- Update indicatif requirement from 0.14 to 0.15 (#200) @dependabot-preview

## [0.9.4] - 2020-06-11

### Fixed

- Invalid read: `Multi::close` called twice (#198) @DBLouis
- Change all tracing spans to `Trace` level to reduce log noise (#195) @sagebind

### Changed

- Update analysis CI job (#197) @sagebind
- Expand first readme example to full program (#194) @sagebind

### Dependency Updates

- Update Public Suffix List to fe4225d (#196) @sagebind

## [0.9.3] - 2020-05-24

### Fixed

- Fix built-in user agent overriding client default headers. (#191) @sagebind
- Fix incorrectly named function parameter. (#188) @DBLouis

### Changed

- Emit all logs as [tracing](https://crates.io/crates/tracing) events, maintaining backward compatability with [log](https://crates.io/crates/log) records. This has the benefit of optionally providing better diagnostics of HTTP requests when using a tracing subscriber. (#170) @sagebind

## [0.9.2] - 2020-05-10

### Added

- Add the ability to set default outgoing header values to include on all requests when building a custom client. Headers set on a request always take precedence over defaults. (#180, #181, #186) @ansrivas

### Fixed

- Fix code test coverage analysis no longer working. (#183, #187) @sagebind

### Dependency Updates

- Update Public Suffix List to c1d5b3c (#184) @sagebind
- Update Public Suffix List to 17ca522 (#177) @sagebind
- Update mockito requirement from 0.23 to 0.25 (#178) @dependabot-preview

## [0.9.1] - 2020-03-11

### Changed

- Implement `Send` for the opaque `Future` type returned by `ResponseExt::text_async` whenever the response body also implements `Send`. (#173, #175)

### Dependency Updates

- Update Public Suffix List to 9afbb37 (#174)

## [0.9.0] - 2020-03-05

**Welcome to a new decade!**

This release includes a number of API improvements and features, as well as a couple bug fixes. The API changes improve ease of use and ergonomics, incorporating the new [0.2 version of the `http` crate](https://github.com/hyperium/http/releases/tag/v0.2.0), as well as reduces the number of confusing types to help make finding what you are looking for easier in the documentation.

### Breaking Changes

- Request configuration is now done via the `Configurable` trait, which unifies the old methods from `HttpClientBuilder` and `RequestBuilderExt` into one place. The old methods have been removed, but most programs should compile without changes if the prelude is imported. (#48, #135)
- Multiple breaking API improvements to `Body` (#143):
    - `Body::reader` and `Body::reader_sized` have been renamed to `Body::from_reader` and `Body::from_reader_sized`, respectively.
    - `Body::bytes` has been replaced with `Body::from_maybe_shared`, which uses type downcasting to accept a `Bytes` if given without having the `bytes` crate being part of the public API.
    - `Body` no longer implements `From<Bytes>` for the reason above.
    - `Body::text`, `Body::text_async`, and `Body::json` have all been removed in favor of the equivalent methods provided by `ResponseExt`. This was done because the body alone is often not enough information to decode responses in a correct manner. (#142)
- Various improvements to request config bounds that accept more argument types.
- The `cookies` feature is no longer enabled by default. (#141)
- Creating a `Body` from an `AsyncRead` must now be `Sync` so that `Body` implements `Sync`. (#172)
- Response text decoding methods are now behind the `text-decoding` feature, enabled by default. (#156)

### Added

- Handle more than just UTF-8 when decoding a response into text. (#90, #156)
- Add ability to bind to specific network interface. (#151, #155)
- Add ability to pre-populate DNS lookups with predefined mappings. (#114)
- Add Isahc logo to documentation. (#138) @jmjoy

### Fixed

- `Body::is_empty` should not return true for a zero-length body that is present (as opposed to _no body_). (#144)
- Upgrade curl-sys to fix static linking issues with older versions of macOS (#68, #169)
- Fix doctests and run cargo fmt (#160) @ggriffiniii

### Changed

- Improve proxy handling test coverage (#167)
- Make default `VersionNegotiation` more conservative (#159, #164)
- Set up code coverage analysis via grcov (#165)

### Dependency Updates

- Upgrade http from 0.1 to 0.2 (#135)
- Update Public Suffix List to 11f4542
  (#166)
- Update indicatif requirement from 0.13 to 0.14 (#162) @dependabot-preview
- Update mockito requirement from 0.22 to 0.23 (#163) @dependabot-preview
- Update Public Suffix List to d73f42f
  (#149)
- Update bytes requirement from 0.4 to 0.5 (#132) @dependabot-preview
- Update Public Suffix List to a406942
  (#137)

## [0.8.2] - 2019-12-06

### Changed

- Don't ask for default features in `futures-util` because we do not use them. (#134) @jakobhellermann
- Update parking_lot requirement from 0.9 to 0.10 (#133) @dependabot-preview

## [0.8.1] - 2019-11-26

### Fixed

- Only request upgrade to HTTP/2 if it is actually available. (#131) @sagebind

## [0.8.0] - 2019-11-23

This release includes an upgrade to the much awaited futures 0.3, as well as some great new features and a few small breaking improvements to the API.

### Breaking Changes

- Upgrade from futures-preview to futures 0.3. Largely the same as futures-preview, but a breaking change due to the crate switch. (#116) @sagebind
- The `preferred_http_version()` method has been removed in favor of a new `VersionNegotiation` API with more robust configuration, including support for HTTP/2 Prior Knowledge. Generally this will be a mechanical migration from `preferred_http_version()` to `version_negotiation()`. If you were previously passing in `Version::HTTP_2` in order to enable HTTP/2 on insecure requests, you can now remove this as Isahc will include an `Upgrade` header and switch to HTTP/2 automatically by default. (#129) @sagebind
- A couple various boolean SSL options have been replaced with an `ssl_options()` method that permits you to set multiple flags with greater granularity and control. (#128) @sagebind

### Added

- Added options for setting a CA certificate, disabling certificate revocation checks, and disabling proxies globally or for specific hosts. (#124) @ohadravid
- Document json feature in examples. (#121) @gbip
- Add a new API for HTTP auth. This includes a new crate feature `spnego` which allows you to configure HTTP Negotiate. Basic and digest auth are also supported. (#115) @sagebind

### Changed

- Update mockito requirement from 0.21 to 0.22 (#123) @dependabot-preview
- Update crossbeam-utils requirement from 0.6 to 0.7 (#117) @dependabot-preview
- Update indicatif requirement from 0.12 to 0.13 (#119) @dependabot-preview
- Update crossbeam-channel requirement from 0.3 to 0.4 (#120) @dependabot-preview

## [0.7.6] - 2019-11-08

### Added

- Add a new metrics API for inspecting various request timing information. (#47, #88) @sagebind

### Changed

- Update Public Suffix List to b566870 (#112) @sagebind

## [0.7.5] - 2019-11-04

This release adds several new options for configuring connection behavior on an `HttpClient`, as well as a couple important bug fixes.

### Added

- Add options for configuring the connection cache (#95)
- Add option to configure DNS caching per client (#96)
- Add connection limit options for clients (#92)

### Fixed

- Fix agent shutdown caused by unpause errors (#97, #101)
- Keep all instances of the same response header (#100, #107) @alexcormier
- Fix auto referer not setting referer header (#91)

### Changed

- Add better error messaging and logging around agent disconnect (#99)
- Update mockito requirement from 0.20 to 0.21 (#85)

## [0.7.4] - 2019-09-25

### Fixed

- Fix parsing headers with a colon in their value. Previously headers like `Location: https://example.org` were being truncated to `Location: https`. (#82) @puckipedia

### Changed

- Update env_logger requirement from 0.6 to 0.7. (#80)

## [0.7.3] - 2019-09-06

### Added

- Add `ResponseExt::effective_uri` for retrieving the last visited URI during a single request-response cycle. (#74)

### Fixed

- Fix panic due to broken implementation in cookie jar middleware. (#70)

## [0.7.2] - 2019-09-05

### Added

- Add the ability to ignore SSL validation. (#71) @jlricon

### Fixed

- Pull in read-after-EOF bugfix in Sluice. (#72, #73)

### Changed

- Update criterion requirement from 0.2 to 0.3 (#67) @dependabot-preview

## [0.7.1] - 2019-08-23

### Changed

- Upgrade futures-preview dependencies to the latest and greatest of 0.3.0-alpha.18.
- `HttpClient::new()` will now block fully until the client is actually initialized. Previously it would block on _some_ things being initialized, and then return once the agent thread has started. This proved to be a bit too unusual, and had a tendency to cause extra delays for the first few requests being sent. This new behavior should feel much more predictable.

### Fixed

- Benchmarking sub-crate once again compiles and runs.

## [0.7.0] - 2019-08-20

### Breaking Changes

- `HttpClient::new()` now returns a `Result<HttpClient, Error>` since creating a client is fallible. Previously this method would panic if instantiation failed, which was not a very API.
- The `Default` implementation for `HttpClient` has been removed, since instantiating it is fallible.

### Fixed

- Dropping an `HttpClient` previously would cause any active transfers created by the client to return EOF after emptying the response body buffer. Now the curl multi handle will be kept alive until all transfers are completed or cancelled. In addition, reading from the response body will return a `ConnectionAborted` error if for some reason the transfer was stopped without finishing. (#65)

### Changed

- Use the latest GitHub Actions beta features for CI. (#56)

## [0.5.5] - 2019-08-03

Put a notice on the old crate of the new project name.

## [0.6.0] - 2019-08-03

### Changed

- The project has been renamed from cHTTP to Isahc! This also includes an adorable new project mascot... (#36, #54)
- The length property for `Body` has changed from an `usize` to an `u64`. `usize` is too small to fit large file sizes on machines that have less than 64 bit pointer size. (#52)
- The `Error::Internal` variant has been removed, as a panic is more suitable for the one situation that previously returned this error.
- The `Error::TooManyConnections` variant has been removed, as it is an artifact from old cHTTP versions. Isahc has no artificial limit on the number of connections that can be used simultaneously outside of system limits.

## [0.5.4] - 2019-08-03

### Fixed

- Use CURLOPT_POSTFIELDSIZE for POST body (#53)

### Changed

- Update mockito requirement from 0.19 to 0.20 (#50)

## [0.5.3] - 2019-07-26

### Fixed

- Fix dependency issues with `futures-util-preview` where not including `futures-preview` downstream would result in the `io` feature being missing and causing a compile error.
- Replace an unnecessary use of unsafe with the `pin_mut!` macro from `futures-util-preview`.

## [0.5.2] - 2019-07-23

### Changed

- Remove `futures-executor-preview` from list of dependencies. This was only being used for `block_on` in one place, which now has been replaced with a tiny `join()` method that does effectively the same thing.

## [0.5.1] - 2019-07-23

### Added

- Add `ResponseExt::json` to mirror `Body::json`.

### Improvements

- Implement synchronous API without using `block_on`. (#49)
- Remove a few futures-related dependencies we no longer need. (#49)
- Also include extension traits in the library root module.
- Various documentation cleanup with more examples.

## [0.5.0] - 2019-07-21

This is a huge release for cHTTP that delivers on first-class `async`/`.await` and big API ergonomic improvements! Read @sagebind's [blog post](https://stephencoakley.com/2019/07/22/chttp-0.5-and-the-journey-ahead) if you want to learn even more about the release and the project's direction!

### Breaking Changes

- Rename `Client` to `HttpClient` and `ClientBuilder` to `HttpClientBuilder`. For usability it helps to include the client type in our prelude, but the name `Client` is just a little too generic to do that. Using the name `HttpClient` is much less ambiguous as to what kind of client is in question. It's also a very common type name for HTTP clients and adds to familiarity. (#46)
- The `Options` struct has been removed. Instead, request execution options can be set by using extension methods provided for `http::request::Builder`, or by setting default configuration using methods available on `HttpClientBuilder`. This greatly improves the ergonomics of setting various options, as well as allows you to override specific options piecemeal instead of overriding everything. (#35)
- `Body` is now asynchronous internally. It still implements `Read` for ease of use, but you can no longer create a body from a synchronous reader. This fixes latency issues inside the agent thread, where previously sending and receiving bodies might block the entire event loop and prevent other parallel requests from making progress for a short time.
  This also means that you can optionally read from a response `Body` within an asynchronous application without blocking. (#27)
- The `json` feature no longer uses the json crate, and instead uses serde to automatically deserialize a JSON response to the target type.
- Rename the `options` module to `config`, and make several other redundant modules private.
- Removed `Request` and `Response`

### Added

- Add a new `chttp::prelude` module that can be glob-imported to pull in the core types and traits you'll likely need to work with cHTTP.
- In addition to the existing synchronous `send`, `get`, `post` (etc) methods, asynchronous versions of these methods are now available, suffixed with `_async`. These methods do the exact same things as the synchronous versions, except entirely asynchronously and return a [`Future`](https://doc.rust-lang.org/std/future/trait.Future.html). (#27)
- When creating a request body from a reader, you can now provide a length hint if you know the size of the body ahead of time. When providing a length hint, the hint will be used as the value of the `Content-Length` request header and be sent directly, instead of using a streaming encoding.
- Add convenience methods `copy_to` and `copy_to_file` for downloading a response body to a file or an arbitrary writer.

### Improvements

- The background agent thread that manages active requests has received many changes and improvements and is now fully asynchronous This offers less latency when multiple parallel requests are active and increased overall performance.
- Response buffers now use [sluice](https://docs.rs/sluice) which greatly increases the efficiency of reading from the HTTP response stream.
- Trim the dependency tree of crates that we don't really need, greatly improving both compile times and binary footprint. (#43) @sagebind
- Improve how we handle the [Public Suffix List](https://publicsuffix.org/) by bundling a copy via Git and refreshing it periodically during run time. This also removes the `psl` crate, which has been to compile very slowly. (#40)

### Fixed

- Fix a bug where sending a HEAD request might hang waiting on the response until the server closes the connection.
- Fix various potential bugs in how certain HTTP verbs are handled by using explicit curl options for a verb if one exists.
- Fix some warning emitted by Clippy.

### Changed

- Dropping a `Client` will now block until the associated agent thread shuts down. If the agent thread errors, it will propagate back to the main thread.
- Replace usages of Rouille with [Mockito](https://github.com/lipanski/mockito) in integration tests, as it has a lot of nice convenience methods and is a little lighter on the dependencies.

## [0.5.0-alpha.3] - 2019-07-20

### Breaking Changes

- Rename `Client` to `HttpClient` and `ClientBuilder` to `HttpClientBuilder`. For usability it helps to include the client type in our prelude, but the name `Client` is just a little too generic to do that. Using the name `HttpClient` is much less ambiguous as to what kind of client is in question. It's also a very common type name for HTTP clients and adds to familiarity. (#46)

### Added

- Make `ResponseFuture` part of public API.
- Add convenience method for downloading to a file.

### Fixed

- Fix a bug where sending a HEAD request might hang waiting on the response until the server closes the connection.
- Fix various potential bugs in how certain HTTP verbs are handled by using explicit curl options for a verb if one exists.
- Fix some warning emitted by Clippy.

### Changed

- Update `parking_lot` requirement from 0.8 to 0.9 (#45) @dependabot-preview
- Replace usages of rouille with mockito in integration tests, as it has a lot of nice convenience methods and is a little lighter on the dependencies.

## [0.5.0-alpha.2] - 2019-07-15

### Changed

- fix: Refresh PSL during run time (#40) @sagebind
- Dependency tree trimming (#43) @sagebind
- Run tests against both nightly and stable (#38) @sagebind
- Move benchmarks into their own crate (#39) @sagebind

## [0.5.0-alpha.1] - 2019-07-05

This is the first alpha release for version 0.5! There are a lot of things in this release, but the primary changes include improvments to the API and integration with [`std::future::Future`](https://doc.rust-lang.org/std/future/trait.Future.html), which was just recently released as stable.

### Added

- In addition to the existing synchronous `send`, `get`, `post` (etc) methods, asynchronous versions of these methods are now available, suffixed with `_async`. These methods do the exact same things as the synchronous versions, except entirely asynchronously. (#27)
- When creating a request body from a reader, you can now provide a length hint if you know the size of the body ahead of time. When providing a length hint, the hint will be used as the value of the `Content-Length` request header and be sent directly, instead of using a streaming encoding.

### Improvements

- The background agent thread that manages active requests has received many changes and improvements and is now fully asynchronous This offers less latency when multiple parallel requests are active and increased overall performance.
- Response buffers now use [sluice](https://docs.rs/sluice) which greatly increases the efficiency of reading from the HTTP response stream.

### Changed

- The `Options` struct has been removed. Instead, request execution options can be set by using extension methods provided for `http::request::Builder`, or by setting default configuration using methods available on `ClientBuilder`. This greatly improves the ergonomics of setting various options, as well as allows you to override specific options picemeal instead of overriding everything. (#35)
- `Body` is now asynchronous internally. It still implements `Read` for ease of use, but you can no longer create a body from a synchronous reader. This fixes latency issues inside the agent thread, where previously sending and receiving bodies might block the entire event loop and prevent other parallel requests from making progress for a short time.
  This also means that you can optionally read from a response `Body` within an asynchronous application without blocking. (#27)
- Dropping a `Client` will now block until the associated agent thread shuts down. If the agent thread errors, it will propagate back to the main thread.

## [0.4.5] - 2019-07-02

### Security Fixes

- Upgrade the bundled version of libcurl from 7.64.1 to 7.65.1 to address security vulnerability [CVE-2019-5436](https://nvd.nist.gov/vuln/detail/CVE-2019-5436) filed against that version. (#34, #37)

## [0.4.4] - 2019-06-14

### Fixed

- Fix `Canceled` errors being returned when compiled in release mode. (#30, #32)

## [0.4.3] - 2019-06-11

### Changed

- libcurl will now be linked statically by default instead of using the system libcurl when available. Sometimes when using the platform provided libcurl there can be subtle differences between systems that can be hard to track down. This will result in more consistent behavior between platforms that is easier to maintain. If you need to use the platform libcurl specifically, you can disable the new `static-curl` crate feature (enabled by default).

## [0.4.2] - 2019-04-05

- Fix compile issues in the agent notify channel on Windows.

## [0.4.1] - 2019-02-28

- Add additional options for SSL/TLS. You can now override the list of acceptable ciphers to use in an SSL/TLS connection, and also provide a custom client certificate to use.

## [0.4.0] - 2019-02-19

- Reduced API surface area of `Body`, removed `Seek` implementation. In the future, request and response body may be `AsyncRead` instead, or even a trait.
- `Body` can be "resettable" instead of seekable, which will now be used if curl requests to seek to the beginning of the request body stream.
- Small optimization in handling curl multi messages to reduce allocations.

## [0.3.1] - 2019-01-18

- Add some new debug logging and assertions that point out unexpected behavior.
- Add more examples of how cHTTP can be used.
- Update source code to use Rust 2018 edition conventions.

## [0.3.0] - 2018-12-06

- Add a new in-memory cookie jar system for preserving cookies between requests. Use `.with_cookies()` on the client builder to enable cookie management for a client. This feature is put behind the `cookies` feature flag, which is enabled by default.
- Add a new unstable _middleware_ API, which allows you to apply transformations to every client request. You must enable the `middleware-api` feature flag to access it.
- Add a new unstable futures-based API for sending requests asynchronously. You must enable the `async-api` feature flag to access it. This feature will likely not be stabilized until futures are stabilized in the standard library.
- Requests will now include a default user agent if an explicit `User-Agent` header is not set.
- HTTP/2 support can now be disabled by removing the `http2` feature flag (enabled by default).

## [0.2.4] - 2018-11-02

- Add a `version()` function, which returns a human-readable string containing the runtime version of cHTTP and important dependencies. Helpful for debugging endeavors.

## [0.2.3] - 2018-10-31

- Enable curl's built-in gzip and zlib encoding decompression.

## [0.2.2] - 2018-09-18

- Fix following redirect policies not being respected correctly.

## [0.2.1] - 2018-09-15

- Enable HTTP/2 support.
- Apply a workaround for a potential bug in libcurl concerning timeouts in the agent event loop.

## [0.2.0] - 2018-09-13

- Refactor the internals of cHTTP to be "closer to the metal", with a single curl multi handle running in a background thread per client that multiplexes all requests. This improves connection pooling and reduces memory usage, and has only minimal public API changes. This also opens the door to providing an async API in the future. (#5)
- Redesign `Body` public API.
- Include a `Content-Length` header automatically if the request body size is known.
- Add shortcut functions for sending `HEAD` requests.
- Allow users to pass in `Options` attached to individual requests as an extension, eliminating the need to create a custom client just for a simple option.
- Add `with_` methods to `Options`, making it much more ergonomic to create instances with just a few options specified.
- Add options for max upload/download speed.
- Support sending any `Request` with a body that implements `Into<Body>`.
- Improve debug logging.
- Improve integration tests.

## [0.1.5] - 2018-08-03

- Add wire tracing logs for inspecting raw headers being sent and received.
- Fixed issue where messages from libcurl were being discarded before we could read them. This would cause the client to get stuck in an infinite loop whenever a request would reach a timeout. (#3)

## [0.1.4] - 2018-02-24

- Moved the ring buffer out of the codebase into the `ringtail` crate.

## [0.1.3] - 2018-02-02

- Fixed safety and soundness issues in the ring buffer. (#1, #2)

## [0.1.2] - 2017-12-28

- Client options now support specifying a proxy URL.
- Transport API is now private so the design can be revisited later.

## [0.1.1] - 2017-12-21

- Switched to a custom ring buffer implementation for the response body to improve throughput.

## 0.1.0 - 2017-10-28

- Initial release.

[Unreleased]: https://github.com/sagebind/isahc/compare/isahc-v1.8.0...HEAD
[1.8.0]: https://github.com/sagebind/isahc/compare/1.7.2...isahc-v1.8.0
[1.7.2]: https://github.com/sagebind/isahc/compare/1.7.1...1.7.2
[1.7.1]: https://github.com/sagebind/isahc/compare/1.7.0...1.7.1
[1.7.0]: https://github.com/sagebind/isahc/compare/1.6.0...1.7.0
[1.6.0]: https://github.com/sagebind/isahc/compare/1.5.1...1.6.0
[1.5.1]: https://github.com/sagebind/isahc/compare/1.5.0...1.5.1
[1.5.0]: https://github.com/sagebind/isahc/compare/1.4.1...1.5.0
[1.4.1]: https://github.com/sagebind/isahc/compare/1.4.0...1.4.1
[1.4.0]: https://github.com/sagebind/isahc/compare/1.3.1...1.4.0
[1.3.1]: https://github.com/sagebind/isahc/compare/1.3.0...1.3.1
[1.3.0]: https://github.com/sagebind/isahc/compare/1.2.0...1.3.0
[1.2.0]: https://github.com/sagebind/isahc/compare/1.1.0...1.2.0
[1.1.0]: https://github.com/sagebind/isahc/compare/1.0.3...1.1.0
[1.0.3]: https://github.com/sagebind/isahc/compare/1.0.2...1.0.3
[1.0.2]: https://github.com/sagebind/isahc/compare/1.0.1...1.0.2
[1.0.1]: https://github.com/sagebind/isahc/compare/1.0.0...1.0.1
[1.0.0]: https://github.com/sagebind/isahc/compare/1.0.0-beta.1...1.0.0
[1.0.0-beta.1]: https://github.com/sagebind/isahc/compare/0.9.14...1.0.0-beta.1
[0.9.14]: https://github.com/sagebind/isahc/compare/0.9.13...0.9.14
[0.9.13]: https://github.com/sagebind/isahc/compare/0.9.12...0.9.13
[0.9.12]: https://github.com/sagebind/isahc/compare/0.9.11...0.9.12
[0.9.11]: https://github.com/sagebind/isahc/compare/0.9.10...0.9.11
[0.9.10]: https://github.com/sagebind/isahc/compare/0.9.9...0.9.10
[0.9.9]: https://github.com/sagebind/isahc/compare/0.9.8...0.9.9
[0.9.8]: https://github.com/sagebind/isahc/compare/0.9.7...0.9.8
[0.9.7]: https://github.com/sagebind/isahc/compare/0.9.6...0.9.7
[0.9.6]: https://github.com/sagebind/isahc/compare/0.9.5...0.9.6
[0.9.5]: https://github.com/sagebind/isahc/compare/0.9.4...0.9.5
[0.9.4]: https://github.com/sagebind/isahc/compare/0.9.3...0.9.4
[0.9.3]: https://github.com/sagebind/isahc/compare/0.9.2...0.9.3
[0.9.2]: https://github.com/sagebind/isahc/compare/0.9.1...0.9.2
[0.9.1]: https://github.com/sagebind/isahc/compare/0.9.0...0.9.1
[0.9.0]: https://github.com/sagebind/isahc/compare/0.8.2...0.9.0
[0.8.2]: https://github.com/sagebind/isahc/compare/0.8.1...0.8.2
[0.8.1]: https://github.com/sagebind/isahc/compare/0.8.0...0.8.1
[0.8.0]: https://github.com/sagebind/isahc/compare/0.7.6...0.8.0
[0.7.6]: https://github.com/sagebind/isahc/compare/0.7.5...0.7.6
[0.7.5]: https://github.com/sagebind/isahc/compare/0.7.4...0.7.5
[0.7.4]: https://github.com/sagebind/isahc/compare/0.7.3...0.7.4
[0.7.3]: https://github.com/sagebind/isahc/compare/0.7.2...0.7.3
[0.7.2]: https://github.com/sagebind/isahc/compare/0.7.1...0.7.2
[0.7.1]: https://github.com/sagebind/isahc/compare/0.7.0...0.7.1
[0.7.0]: https://github.com/sagebind/isahc/compare/0.5.5...0.7.0
[0.5.5]: https://github.com/sagebind/isahc/compare/0.6.0...0.5.5
[0.6.0]: https://github.com/sagebind/isahc/compare/0.5.4...0.6.0
[0.5.4]: https://github.com/sagebind/isahc/compare/0.5.3...0.5.4
[0.5.3]: https://github.com/sagebind/isahc/compare/0.5.2...0.5.3
[0.5.2]: https://github.com/sagebind/isahc/compare/0.5.1...0.5.2
[0.5.1]: https://github.com/sagebind/isahc/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/sagebind/isahc/compare/0.5.0-alpha.3...0.5.0
[0.5.0-alpha.3]: https://github.com/sagebind/isahc/compare/0.5.0-alpha.2...0.5.0-alpha.3
[0.5.0-alpha.2]: https://github.com/sagebind/isahc/compare/0.5.0-alpha.1...0.5.0-alpha.2
[0.5.0-alpha.1]: https://github.com/sagebind/isahc/compare/0.4.5...0.5.0-alpha.1
[0.4.5]: https://github.com/sagebind/isahc/compare/0.4.4...0.4.5
[0.4.4]: https://github.com/sagebind/isahc/compare/0.4.3...0.4.4
[0.4.3]: https://github.com/sagebind/isahc/compare/0.4.2...0.4.3
[0.4.2]: https://github.com/sagebind/isahc/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/sagebind/isahc/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/sagebind/isahc/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/sagebind/isahc/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/sagebind/isahc/compare/0.2.4...0.3.0
[0.2.4]: https://github.com/sagebind/isahc/compare/0.2.3...0.2.4
[0.2.3]: https://github.com/sagebind/isahc/compare/0.2.2...0.2.3
[0.2.2]: https://github.com/sagebind/isahc/compare/0.2.1...0.2.2
[0.2.1]: https://github.com/sagebind/isahc/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/sagebind/isahc/compare/0.1.5...0.2.0
[0.1.5]: https://github.com/sagebind/isahc/compare/0.1.4...0.1.5
[0.1.4]: https://github.com/sagebind/isahc/compare/0.1.3...0.1.4
[0.1.3]: https://github.com/sagebind/isahc/compare/0.1.2...0.1.3
[0.1.2]: https://github.com/sagebind/isahc/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/sagebind/isahc/compare/0.1.0...0.1.1
