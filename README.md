# Datastar Rust SDK

[![Crates.io](https://img.shields.io/crates/v/datastar.svg)](https://crates.io/crates/datastar)
[![Documentation](https://docs.rs/datastar/badge.svg)](https://docs.rs/datastar)
[![CI](https://github.com/starfederation/datastar-rust/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/starfederation/datastar-rust/actions/workflows/CI.yml)
![MSRV](https://img.shields.io/badge/MSRV-1.89.0-blue.svg)
[![License](https://img.shields.io/crates/l/datastar.svg)](./LICENSE.md)

An implementation of the [Datastar] SDK in Rust
with framework integration for [Actix Web], [Axum], [Rocket], and [Warp].

Supports Datastar **1.0.4**. Its SSE event format and SDK defaults are unchanged
from 1.0.3.

Rust web frameworks own SSE stream lifecycle and backpressure, so this SDK
intentionally returns framework-native events instead of providing a
`ServerSentEventGenerator`.

[Rama](https://github.com/plabayo/rama) has its own SDK implementation defined as a [Rama module for Datastar](https://ramaproxy.org/docs/rama/http/sse/datastar/index.html) as can be seen in action in [this example](https://github.com/plabayo/rama/blob/main/examples/http_sse_datastar_hello.rs).

# Usage

Runnable examples for every supported framework can be found in
[`examples`](./examples).

## QUERY requests

Datastar 1.0.4 adds `@query('/endpoint')`, which sends a `QUERY` request with
signals in the JSON body. GET and DELETE continue to send signals in the
`datastar` URL query parameter.

- **Actix Web:** `ReadSignals<T>` reads QUERY bodies. Route the method with
  `web::method(Method::from_bytes(b"QUERY")?)`; the built-in method guards do
  not cover custom methods. See
  [`actix-test-suite`](./examples/actix-test-suite.rs).
- **Axum:** `ReadSignals<T>` reads QUERY bodies. Route the method using
  `axum::routing::any` or a method-router fallback; `MethodFilter` does not
  support custom methods. See [`axum-test-suite`](./examples/axum-test-suite.rs)
  for a fallback that accepts QUERY and rejects other unhandled methods.
- **Warp:** `read_signals::<T>()` reads QUERY bodies. Use `warp::method()` to
  match `method.as_str() == "QUERY"`, as in
  [`warp-test-suite`](./examples/warp-test-suite.rs).
- **Rocket 0.5:** custom HTTP methods are rejected before reaching a handler.
  Use `@post('/endpoint')` and a `ReadSignals<T>` data guard instead.

The new free Datastar Rocket browser bundle is unrelated to Rust's Rocket
framework. It uses the same SDK events. The other
[1.0.4 release changes](https://github.com/starfederation/datastar/releases/tag/v1.0.4),
including CSP nonce aliases, request cancellation, and signal reactivity fixes,
are handled by the browser bundle.

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

## SDK conformance

Run the official Datastar SDK suite against Actix Web, Axum, Rocket, and Warp
with Go:

```sh
make test-datastar-sdk
```

Or run the official test runner in Docker:

```sh
make test-datastar-sdk-docker
```

The runner defaults to the suite at Datastar 1.0.4 and also checks QUERY routing
and responses for Actix Web, Axum, and Warp. Set `DATASTAR_SDK_TEST_VERSION=latest` to test
against the newest upstream suite.

[Datastar]: https://data-star.dev
[Actix Web]: https://github.com/actix/actix-web
[Axum]: https://github.com/tokio-rs/axum
[Rocket]: https://github.com/rwf2/rocket
[Warp]: https://github.com/seanmonstar/warp
