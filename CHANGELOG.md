# Changelog

## Unreleased

- Add an Actix Web integration behind the `actix` feature: a streaming `Sse`
  responder with owned and borrowed conversions from every event type, a
  `ReadSignals` extractor (GET/DELETE query parameter, JSON body otherwise,
  including QUERY), and Datastar header constants.
- Add Actix Web hello, activity-feed, live-reload, and test-suite examples, and
  run the official SDK suite against Actix Web in CI.

## 0.4.1 - 2026-09-29

- Sync examples with Datastar 1.0.4 and verify QUERY body signal extraction
  and routing for Axum and Warp. Rocket 0.5 cannot accept custom HTTP methods;
  use POST for body signals instead.
- Datastar's SSE format and SDK defaults are unchanged from 1.0.3. The free
  Datastar Rocket browser bundle and CSP, cancellation, and reactivity fixes
  require no additional SDK event types or options.
- Make the official SDK suite reusable locally with native Go or Docker.
- Pin the default suite to Datastar 1.0.4 and add QUERY HTTP checks.
- Update dependencies while retaining the Rust 1.89 MSRV.

_Thanks to the Datastar maintainers for the 1.0.3 and 1.0.4 releases._

## 0.4.0 - 2026-08-21

- Sync with Datastar 1.0.2, adding namespaces and scoped view transitions. The
  new `PatchElements` fields make 0.3 struct literals source-incompatible.
- Add Warp (#9) and Rocket signal extraction; support Datastar request headers
  (#6), DELETE queries (#12), and missing optional query signals (#11).
- Improve framework-native SSE serialization and run the official SDK suite
  against Axum, Rocket, and Warp.
- Raise the MSRV to 1.89, remove `http2` (feature), and minimize dependency features.

_Thanks to @jtornert (#4), @tkr-sh (#6), @larrydewey (#8), @jrey8343 (#9),
@bencroker (#12), @MagnumTrader for the namespace proposal (#13), and
@benjibigboss (#14)._

## 0.3.1 - 2025-08-05

- Add borrowed and owned conversions to Axum and Rocket SSE events.
- Add Axum activity-feed (#2) and Rocket channel (#3) examples.

_Thanks to @VimCommando (#2) and @maacl (#3)._

## 0.3.0 - 2025-07-17

- Refactor the API for Datastar 1.0 RC1
  (starfederation/datastar#952, starfederation/datastar#982).
- Add the official Axum SDK test runner (starfederation/datastar#969).
- Move Rama support into Rama's own Datastar module.

_Thanks to the Datastar maintainers and everyone who tested the RC API._

## 0.2.1 - 2025-05-13

- Update the Rama integration to stable Rama 0.2
  (starfederation/datastar#888).

_Thanks to the Rama community for testing the integration._

## 0.2.0 - 2025-05-03

- Add direct framework event conversions, removing the intermediate `.into()`
  call (starfederation/datastar#856).
- Improve framework integrations and diagnostics
  (starfederation/datastar#872).

_Thanks to early SDK users for their feedback._

## 0.1.3 - 2025-04-18

- Update the experimental Rama integration to alpha.12.

_Thanks to the Rama users who tested the early integration._

## 0.1.2 - 2025-04-18

- Update the experimental Rama integration to alpha.11.

_Thanks to the Rama users who tested the early integration._

## 0.1.1 - 2025-04-18

- Add experimental Rama support (starfederation/datastar#847).

_Thanks to the Rama community for the early feedback._

## 0.1.0 - 2025-04-18

- Initial Rust SDK release with core events plus Axum and Rocket integrations
  (starfederation/datastar#520, starfederation/datastar#558).
- Add initial SDK fixes and tooling (starfederation/datastar#632,
  starfederation/datastar#854).

_Thanks to @nonnorm and the original Rust SDK contributors._
