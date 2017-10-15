# cHTTP
The practical HTTP client that is fun to use.

cHTTP provides a clean and easy-to-use interface around the venerable [libcurl]. Here are some of the features that are currently available:

- Connection pooling and reuse.
- Respone body streaming.
- Request body uploading from memory or a stream.
- Tweakable redirect policy.
- TCP socket configuration.
- Uses the future standard Rust [http] interface for requests and responses.

## Why [libcurl]?
Not everything needs to be re-invented! For typical use cases, [libcurl] is a fantastic choice for making web requests. It's fast, reliable, well supported, and isn't going away any time soon.

It has a reputation for having an unusual API that is sometimes tricky to use, but hey, that's why this library exists.

## Requirements
On Linux:

- libcurl 7.24.0 or newer
- OpenSSL 1.0.1, 1.0.2, 1.1.0, or LibreSSL

On Windows and macOS:

- TBD


## License
This library is licensed under the MIT license. See the [LICENSE](LICENSE) file for details.


[http]: https://github.com/hyperium/http
[libcurl]: https://curl.haxx.se/libcurl/
