# Datastar Rust SDK

An implementation of the [Datastar] SDK in Rust
with framework integration for [Axum], [Rocket], and [Warp].

[Rama](https://github.com/plabayo/rama) has its own SDK implementation defined as a [Rama module for Datastar](https://ramaproxy.org/docs/rama/http/sse/datastar/index.html) as can be seen in action in [this example](https://github.com/plabayo/rama/blob/main/examples/http_sse_datastar_hello.rs).

# Usage

Runnable examples for every supported framework can be found in
[`examples`](./examples).

## Long-lived streams

A long-lived SSE handler can receive updates from any number of application
tasks. Publish server-side state changes through channels such as
`tokio::sync::watch` or `tokio::sync::broadcast`, then wait on all receivers in
the handler with `tokio::select!`. Requests that change state only need to
publish an update; the existing SSE response remains open and sends the
corresponding Datastar event.

The [`axum-watch`](./examples/axum-watch.rs) example demonstrates three
independent `watch` channels feeding one SSE response. The
[`rocket-hello-channel`](./examples/rocket-hello-channel.rs) example shows the
same basic pattern with Rocket.

[Datastar]: https://data-star.dev
[Axum]: https://github.com/tokio-rs/axum
[Rocket]: https://github.com/rwf2/rocket
[Warp]: https://github.com/seanmonstar/warp
