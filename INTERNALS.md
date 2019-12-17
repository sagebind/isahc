# Isahc Internals

This document describes, at a high level, how Isahc works and how the code is structured. If you are looking to contribute to Isahc then you might find the information here helpful to you to get started.

## Asynchronous Core

Isahc is designed to be asynchronous from the ground-up for more than just users that want to `.await` their responses. An asynchronous design has many advantages even when being used in a synchronous program.

Imagine you have a web service that may need to make multiple requests to another downstream service in order to fulfill a request. Since these requests could potentially be parallel, you need to be able to support sending multiple requests at the same time, potentially many of them. There are generally three solutions to this problem:

- Create a brand new connection for every request that operates independently from other requests. This is simple to implement, but you cannot take advantage of pipelining, HTTP/2+ advancements, or persistent connections to reduce latency.
- Create a fixed-size pool of connections and threads where requests can be executed. This traditional approach reduces latency by re-using connections and allows you to make concurrent background requests, but is limited by the size of the pool. Choosing a pool size is difficult, as larger sizes give you more concurrency, but also use more memory.
- Multiplex many connections at once using an event loop. This has the same advantages as the previous solution, except you can make as many concurrent requests as your system will allow with minimal resource usage.

While the last solution can be the most difficult to implement, the advantages are clear even for for traditional applications. This is the approach Isahc takes.

## Request Lifecycle

To send a new request, a curl easy handle is created to be driven to completion by an HTTP client instance. To avoid exposing any underlying curl details, and to allow us to present our own ergonomic API, users can construct their own `http::Request` struct which we use as a specification for how to configure an easy handle.

Once the user has an `http::Request` they wish to send, we create a new [_request handler_](#request-handlers) for the request, which receives callbacks from curl about a single request and manages that request's state. We then send the request handler over a channel to an [_agent thread_](#agent-threads), where the request will be driven to completion.

Once response headers are received from the server, the request handler completes a future with a stream of the response body, signalling that the response has been received. The user can continue to read from the response body stream until the end, which signals the end of the response lifecycle. The request handler is then closed and discarded.

## Request Handlers

Request handlers are responsible for keeping track of a request's state at all times, and is also responsible for completing a future once all the response headers are received.

The request handler is tied to the easy handle that corresponds to the request.

## Agent Threads

Each HTTP client instance holds a strong reference to a single background thread, called an _agent thread_. All calls to curl while a request is in flight happen inside this thread, which multiplexes many requests using curl's [multi interface]. This allows us to take advantage of all curl's pooling features, including:

- Connection pooling and reuse
- Multiplexing multiple requests on a single connection
- DNS caching

Communicating with an agent thread is done via message passing. Whenever a new agent is spun up, a corresponding _agent handle_ is also created. The handle maintains a channel with the agent thread which is used to send various messages that allow you to control agent behavior.

The relationship between an agent thread and its handle is very tight. Should the agent thread ever panic or disconnect from the message channel, the handle will ensure that any errors will bubble up to the parent thread. Dropping the handle will also ask the thread nicely to shut down, and block the caller until it does so. In addition, should the handle disappear without warning, the agent thread will automatically shut itself down with an error.

### Disadvantages

There are disadvantages to this design, though they seem to be worth the tradeoffs. The biggest disadvantage is that an extra background thread is created unconditionally. Threads can be a bit heavyweight in some scenarios, and causes initializing new clients to be quite slow.

Another disadvantage is that we cannot take full advantage of multi-core systems. Curl's multi interface does not seem to be conducive to parallelization, and the sacrifices that would have to be made for a multi-threaded design just might negate any performance that we would hope to gain. The only clear way to parallelize curl would be to create N multi handles, one per core, and then share connection pools between them using the [share interface]. For thread safety, we would have to configure curl to use mutexes whenever it accesses shared resources.

This of course is doable, but it magnifies the first advantage and also increases complexity in order to parallelize something that is primarily I/O bound.


[multi interface]: https://curl.haxx.se/libcurl/c/libcurl-multi.html
[share interface]: https://curl.haxx.se/libcurl/c/libcurl-share.html
