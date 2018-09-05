# Changelog

## 0.2.0 - 2018-09-04

- Refactor the internals of cHTTP to be "closer to the metal", with a single curl multi handle running in a background thread per client that multiplexes all requests. This improves connection pooling and reduces memory usage, and has only minimal public API changes. This also opens the door to providing an async API in the future. (#5)
- Redesign `Body` public API.
- Include a `Content-Length` header automatically if the request body size is known.
- Add shortcut functions for sending `HEAD` requests.
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
