# Datastar Rust SDK

[![Crates.io](https://img.shields.io/crates/v/datastar.svg)](https://crates.io/crates/datastar)
[![Documentation](https://docs.rs/datastar/badge.svg)](https://docs.rs/datastar)
[![CI](https://github.com/starfederation/datastar-rust/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/starfederation/datastar-rust/actions/workflows/CI.yml)
![MSRV](https://img.shields.io/badge/MSRV-1.89.0-blue.svg)
[![License](https://img.shields.io/crates/l/datastar.svg)](./LICENSE.md)

An implementation of the [Datastar] SDK in Rust
with framework integration for [Axum], [Rocket], and [Warp].

Rust web frameworks own SSE stream lifecycle and backpressure, so this SDK
intentionally returns framework-native events instead of providing a
`ServerSentEventGenerator`.

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
