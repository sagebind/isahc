# Changelog

## 0.3.0 - 2018-12-05

- Add a new in-memory cookie jar system for preserving cookies between requests. Use `.with_cookies()` on the client builder to enable cookie management for a client. This feature is put behind the `cookies` feature flag, which is enabled by default.
- Add a new unstable _middleware_ API, which allows you to apply transformations to every client request. You must enable the `middleware-api` feature flag to access it.
- Add a new unstable futures-based API for sending requests asynchronously. You must enable the `async-api` feature flag to access it. This feature will likely not be stabilized until futures are stabilized in the standard library.
- Requests will now include a default user agent if an explicit `User-Agent` header is not set.
- HTTP/2 support can now be disabled by removing the `http2` feature flag (enabled by default).

## 0.2.4 - 2018-11-01

- Add a `version()` function, which returns a human-readable string containing the runtime version of cHTTP and important dependencies. Helpful for debugging endeavors.

## 0.2.3 - 2018-10-30

- Enable curl's built-in gzip and zlib encoding decompression.

## 0.2.2 - 2018-09-17

- Fix following redirect policies not being respected correctly.

## 0.2.1 - 2018-09-15

- Enable HTTP/2 support.
- Apply a workaround for a potential bug in libcurl concerning timeouts in the agent event loop.

## 0.2.0 - 2018-09-12

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

## 0.1.5 - 2018-08-03

- Add wire tracing logs for inspecting raw headers being sent and received.
- Fixed issue where messages from libcurl were being discarded before we could read them. This would cause the client to get stuck in an infinite loop whenever a request would reach a timeout. (#3)

## 0.1.4 - 2018-02-24

- Moved the ring buffer out of the codebase into the `ringtail` crate.

## 0.1.3 - 2018-02-01

- Fixed safety and soundness issues in the ring buffer. (#1, #2)

## 0.1.2 - 2017-12-28

- Client options now support specifying a proxy URL.
- Transport API is now private so the design can be revisited later.

## 0.1.1 - 2017-12-21

- Switched to a custom ring buffer implementation for the response body to improve throughput.

## 0.1.0 - 2017-10-28

- Initial release.
