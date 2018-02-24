# Changelog

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
