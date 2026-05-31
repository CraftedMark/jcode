# JCode Mobile PRD

> Status: Draft v0.2
> Updated: 2026-05-31
> Product direction: Native iOS host, shared Rust mobile core

## Summary

JCode Mobile is a native iOS companion app for controlling and monitoring a
desktop `jcode` session from an iPhone. The phone is a rich touch client: it
pairs with a local Mac gateway, shows active agent work, streams chat and tool
activity, lets the user send instructions, and eventually handles approvals,
notifications, speech, camera, and ambient monitoring.

The iPhone does not run tools, shell commands, MCP servers, or local model
inference. Those remain on the desktop `jcode` server. To avoid building the app
twice, app behavior must live in the shared Rust mobile core and be exercised by
the Linux-native simulator. Swift owns the iOS platform shell: UI hosting,
Keychain, camera, speech, push notifications, haptics, and OS lifecycle.

## Problem

`jcode` is powerful when the user is at the terminal, but long-running agent work
does not fit a terminal-only workflow. The user needs to step away from the Mac
while still being able to see progress, answer blocking prompts, send follow-up
instructions, and know when work is ready to review.

The iOS sandbox prevents the phone from becoming the execution environment, so
the correct product is a reliable remote control and status surface for the
desktop agent runtime.

## Goals

- Pair an iPhone to a desktop `jcode` gateway in under two minutes.
- Reconnect reliably over Tailscale or trusted LAN without re-pairing.
- Show current sessions, transcript history, streaming assistant output, and
  tool progress in a native mobile layout.
- Let the user send messages, cancel or interrupt work, switch sessions, and
  choose models when supported by the server.
- Surface blocking tool approvals and important task state through push
  notifications and Live Activities.
- Keep product behavior in Rust so the simulator, tests, and Swift host share
  the same reducer, protocol interpretation, and semantic UI state.
- Keep the iOS app installable on a physical device with the user's Apple
  Development account.

## Non-Goals

- Running shell commands, filesystem edits, git operations, MCP servers, or LLM
  inference on the phone.
- Replacing the terminal UI or desktop app for heavy editing workflows.
- Building a public cloud service or multi-tenant hosted relay for v1.
- Shipping a full code editor on iOS.
- Maintaining separate Swift-only and Rust-only implementations of the same app
  behavior.

## Primary Users

### Local Developer

Runs `jcode` on a Mac and wants to monitor or steer coding agents while away
from the keyboard.

### Agent Operator

Keeps long-running `jcode` work visible on a desk, approves safe blocking
actions, and quickly notices failed or completed work.

## Product Principles

- **Desktop executes, phone directs.** The phone is the command and awareness
  layer; the Mac remains the capability layer.
- **Rust owns behavior.** State transitions, protocol parsing, chat behavior,
  tool-call state, sessions, model selection, reconnect policy, and semantic UI
  must be in `jcode-mobile-core`.
- **Swift owns platform.** SwiftUI, Keychain, camera, speech, APNs, haptics,
  lifecycle, and native navigation belong in the iOS host.
- **Simulator first.** Every product behavior that can be tested without Apple
  tooling should land in the Rust simulator before relying on device testing.
- **Real device acceptance.** A mobile slice is not complete until the iPhone app
  can build, install, launch, and connect to a reachable gateway.

## Current State

Already implemented:

- SwiftUI iOS project and app shell under `ios/`.
- Pairing client, gateway client, credential storage, QR scanner, speech bridge,
  and image picker prototypes in Swift.
- Desktop gateway endpoints for health, pairing, and WebSocket connection.
- Rust mobile core support for pairing validation, chat send and streaming
  response behavior, and tool-call state transitions.
- Linux-native mobile simulator crate and regression path.
- Physical iPhone build, install, and launch path using the local Apple
  Development team.
- Verified gateway connectivity on port `7643` when `jcode serve` is running.
- Mobile session state, tool-call state, and chat streaming live in
  `jcode-mobile-core` (Swift no longer owns these reducers).
- Rust ↔ Swift FFI bridge: `jcode-mobile-ffi` is linked into the iOS app, and
  the iOS host dispatches state transitions through the Rust core.
- Mobile approvals slice: approval state in core, decision protocol, request
  polling, and gateway wiring for allow/deny actions from the device.
- Mobile notification bridge for surfacing server-side events to the iOS host.
- Hardened pairing (re-pairing supported), stabilized speech permission and
  dictation flow, and stabilized QR-scanner capture queue.
- Model visibility, reconnect/reload diagnostic state, connection transport,
  connection phase, and status detail now flow through `jcode-mobile-core` and
  the Swift bridge for locally verifiable server events.
- Settings diagnostics view shows selected server, app version, server version,
  transport, connection phase, provider, status detail, last disconnect,
  notification status, and gateway health.

Open gaps:

- Remaining semantic UI surfaces and platform lifecycle effects still need to
  be audited so Swift only hosts platform effects and presentation.
- APNs registration, Live Activities, and privacy/lock-screen controls for
  approvals and notifications are not yet wired.
- Diagnostics still need real-device and live-gateway acceptance, including
  safe log excerpts and failure classification beyond the locally replayed
  event set.

## Architecture

### Components

- **iOS Swift host**
  - Renders native SwiftUI views.
  - Stores secrets in Keychain.
  - Handles camera, photo picker, speech, push registration, haptics, and
    foreground/background lifecycle.
  - Executes platform effects requested by Rust and returns platform events.

- **Rust mobile core**
  - Owns `MobileAppState`, `MobileAction`, `MobileEffect`, reducer logic,
    protocol models, chat behavior, tool-call state, sessions, model state,
    reconnect policy, and semantic UI data.
  - Powers both the simulator and iOS host.

- **Desktop gateway**
  - Runs inside or beside `jcode serve`.
  - Exposes `GET /health`, `POST /pair`, and WebSocket `/ws` on port `7643`.
  - Authenticates paired devices and streams session events.

- **Linux mobile simulator**
  - Exercises Rust app behavior without Xcode or an iPhone.
  - Provides deterministic fake backend scenarios for product and regression
    testing.

### Network Model

Preferred connectivity is Tailscale-first. Trusted LAN is acceptable for local
development and early testing. The app should make the active transport and
gateway status obvious to the user.

## Functional Requirements

### 1. Pairing and Onboarding

The user can pair the app with a desktop gateway using host, port, device name,
and pairing code, or by scanning a QR payload generated by `jcode pair`.

Acceptance criteria:

- Host, port, pairing code, and device name validation happens in Rust.
- Pairing errors are classified into actionable states: unreachable gateway,
  expired code, invalid code, unsupported server, and server error.
- Successful pairing stores a long-lived token in Keychain.
- The selected server is persisted and can be reused after app relaunch.
- The UI never asks for a host value containing `:port`; port is a separate
  field.

Edge cases:

- Gateway offline.
- Wrong port.
- Tailscale disconnected.
- Expired or already-used pairing code.
- Duplicate device name.
- Server version too old for the app.

### 2. Server Management

The app can store, select, rename, and remove paired servers.

Acceptance criteria:

- Secrets stay in Keychain.
- Non-secret metadata is versioned and migratable.
- Removing a server deletes its token.
- The app can recover cleanly if Keychain has a token but metadata is missing,
  or metadata exists but the token is missing.

### 3. Connection Lifecycle

The app connects to the selected gateway, subscribes to session state, and
recovers from transient network changes.

Acceptance criteria:

- Rust owns connection intent, visible connection state, reconnect policy, stale
  event handling, and user-facing status.
- Swift owns the socket primitive unless or until Rust networking becomes the
  chosen implementation.
- Foregrounding the app reconnects if needed.
- Backgrounding the app does not corrupt active state.
- Server reload events produce a clear reconnect path.

Edge cases:

- WebSocket drops mid-stream.
- Server restarts while app is backgrounded.
- App receives late events from a stale connection.
- Auth token is revoked.
- Network changes between Wi-Fi, cellular, and Tailscale.

### 4. Chat and Transcript

The user can send messages and see assistant output stream in real time.

Acceptance criteria:

- Rust owns draft validation, user message append, assistant placeholder
  creation, streaming append, text replacement, done/error handling, and history
  mapping.
- Empty or whitespace-only drafts do not send.
- Image or attachment metadata is represented in shared state before sending.
- Failed sends produce a recoverable error without losing the draft when
  appropriate.
- Long transcripts remain scrollable and do not degrade the UI.

### 5. Tool Activity

The app shows tool starts, input, execution output, completion, and failure
states attached to the relevant assistant turn.

Acceptance criteria:

- Rust owns tool-call state and transitions.
- The UI can distinguish pending, streaming, executing, succeeded, failed, and
  cancelled states.
- Long tool output is summarized by default with access to details.
- Unknown tool event shapes are preserved enough for diagnostics.

### 6. Sessions and Models

The app supports active session display, session switching, session resume, and
model visibility or selection when the server supports it.

Acceptance criteria:

- Rust owns session list, active session, session switch effects, current model,
  available models, and model-changed events.
- Switching sessions clears stale in-flight UI state and loads the selected
  transcript.
- Model changes are confirmed by server event, not only optimistic UI state.

### 7. Interrupts and Cancellation

The user can cancel a running response or send a soft interrupt where supported.

Acceptance criteria:

- Cancel and soft interrupt actions are represented in Rust effects.
- Interrupted state is visible in the transcript.
- Placeholder assistant messages are cleaned up correctly on interruption.
- The UI prevents duplicate interrupt commands for the same in-flight turn.

### 8. Approvals

The app can surface blocking approval requests and let the user approve or deny
them.

Acceptance criteria:

- Approval requests include command summary, risk level, workspace context,
  timeout, and allow/deny actions.
- Approvals are scoped to one request unless the server explicitly provides a
  broader policy option.
- Expired approvals cannot be submitted.
- Approvals can be delivered from a foreground view and, later, from push or
  Live Activity surfaces.

### 9. Notifications and Live Activities

The app notifies the user when work completes, fails, or needs attention.

Acceptance criteria:

- Push registration is a Swift platform service exposed to Rust as an effect.
- Notification routing reconnects to the relevant server and session.
- Notifications avoid leaking sensitive command output on the lock screen by
  default.
- Live Activity state is compact, current, and dismissible.

### 10. Speech, Camera, and Attachments

The app supports mobile-native input without putting behavior in Swift-only
logic.

Acceptance criteria:

- Speech recognition remains a Swift platform service, but transcript injection
  and send behavior are Rust actions.
- QR scanning remains native, but QR payload parsing and validation should move
  into Rust where practical.
- Image selection and camera capture remain native, but attachment limits,
  metadata, and send rules should be represented in Rust.

### 11. Diagnostics

The app provides enough diagnostics to debug connection and gateway failures
without Xcode.

Acceptance criteria:

- User-visible status separates DNS/host failure, port closed, pairing failure,
  auth failure, WebSocket disconnect, and server reload.
- A diagnostics view shows selected server, app version, server version,
  transport, last health check, last WebSocket close reason, and safe log
  excerpts.
- Sensitive tokens are never displayed.

## UX Requirements

- First launch should open directly to pairing if no server is configured.
- Returning users should land on the last active session or a clear reconnect
  state.
- The primary screen should make agent state scannable: running, blocked,
  failed, complete, or disconnected.
- Chat, tool activity, and approvals should be reachable without deep
  navigation.
- Controls should be native, compact, and one-handed where possible.
- Error messages should tell the user what to check next, not expose raw
  transport errors first.

## Security Requirements

- Tokens are stored only in Keychain on iOS.
- Pairing codes have short TTLs and are single-use.
- Gateway access requires bearer-token authentication after pairing.
- Device revocation must be supported server-side.
- Lock-screen notifications must avoid sensitive content unless the user opts
  in.
- The app should prefer Tailscale or trusted LAN during early releases; public
  internet exposure requires a separate TLS and threat-model pass.

## Success Metrics

- Median first pairing time under two minutes.
- At least 95% successful reconnects within ten seconds on a stable trusted
  network.
- No known Swift-only implementation of product behavior that is also required
  by the simulator.
- Rust mobile core and simulator tests pass for every merged mobile behavior
  slice.
- Physical iPhone build, install, launch, and gateway health verification pass
  before beta handoff.

## Milestones

### M0: Baseline Device Loop

Status: Done.

- Swift app builds in simulator.
- Swift harness tests pass.
- Rust mobile core and simulator tests pass.
- Physical iPhone build, install, and launch path works.
- Gateway can listen on `0.0.0.0:7643` and return `/health`.

### M1: Shared Core Parity

Status: Locally verified for the current shared-core event set — pairing,
chat send/stream, tool-call state, session state, model visibility, reconnect
diagnostics, connection transport, connection phase, and status detail live in
Rust and are replayed through the Swift bridge.

- ✅ Pairing validation in Rust.
- ✅ Chat send and streaming behavior in Rust.
- ✅ Tool-call state in Rust.
- ✅ Session state moved into core; sessions are preserved across reconnect.
- ✅ Model state and current protocol-event interpretation surfaces land in
  core for available model updates, model changes, connection type, connection
  phase, status detail, disconnect, and reload events.
- ⬜ Add broader simulator scenarios for full success, disconnect, reconnect,
  reload, stale-event, and server-error flows.

### M2: iOS Host Bridge

Status: In progress.

- ✅ Rust ↔ Swift FFI bridge (`jcode-mobile-ffi`) and bridge tooling landed.
- ✅ Rust mobile core linked into the iOS app; module linkage fixed.
- ✅ iOS app state dispatch routed through the Rust core.
- ✅ iOS host forwards connection/model/diagnostic server events through the
  Rust reducer and reflects the resulting state in Settings diagnostics.
- ⬜ Swift executes platform effects for Keychain, HTTP pairing, WebSocket,
  camera, speech, and lifecycle — partially wired, audit remaining surfaces.
- ⬜ Remove duplicated Swift reducers once covered by Rust.

### M3: Real Device Beta

Status: Planned.

- Pair from the installed iPhone app to the desktop gateway.
- Connect, subscribe, send chat, stream output, display tools, switch sessions,
  reconnect, and recover from gateway restart.
- Validate the diagnostics view on a physical iPhone against the live gateway,
  including connection phase, provider/model, status detail, disconnect reason,
  and gateway health.
- Run acceptance on the physical iPhone using the user's Apple Development team.

### M4: Approvals and Notifications

Status: In progress — core approvals slice and the in-app notification bridge
have landed; APNs / Live Activities / privacy controls are still ahead.

- ✅ Approval state, decision protocol, request polling, and gateway wiring
  for allow/deny actions from the device.
- ✅ Mobile notification bridge for in-app surfacing of server events.
- ⬜ Approval-request rendering polish in the iOS host.
- ⬜ APNs registration and server notification routing.
- ⬜ Live Activity for active or blocked work.
- ⬜ Privacy controls for lock-screen content.

### M5: TestFlight Candidate

Status: Planned.

- Harden signing, bundle IDs, entitlements, and release build settings.
- Add beta onboarding and device revocation docs.
- Complete security review for exposed gateway assumptions.
- Package a repeatable release checklist.

## Launch Acceptance Criteria

The first useful beta is ready when:

- A fresh iPhone install can pair to a Mac gateway using `jcode pair`.
- The app can reconnect after force quit, background/foreground, and gateway
  restart.
- The user can send a message and see assistant streaming output.
- Tool-call progress is visible and attached to the correct assistant turn.
- The user can cancel or interrupt running work where supported.
- The app can switch or resume sessions without stale transcript state.
- Diagnostics can explain the common failure cases without Xcode.
- Rust core tests, simulator tests, Swift harness, Xcode build, physical install,
  launch, and gateway health checks all pass.

## Test Plan

Core verification:

- `cargo test -p jcode-mobile-core`
- `cargo test -p jcode-mobile-sim`
- `cargo check -p jcode-mobile-core -p jcode-mobile-sim`

iOS verification:

- `swift run --package-path ios JCodeKitTests`
- Xcode simulator build for `JCodeMobile`
- Physical-device build with automatic signing
- `devicectl` install and launch on the iPhone

Gateway verification:

- Start `jcode serve` with the gateway enabled.
- Verify `GET /health` over localhost and the phone-reachable host address.
- Pair with `jcode pair`.
- Confirm WebSocket subscription, send, stream, disconnect, and reconnect flows.

Regression scenarios:

- Invalid host.
- Closed port.
- Tailscale disconnected.
- Expired pairing code.
- Revoked token.
- Server reload during active stream.
- Large transcript.
- Tool failure.
- Approval timeout.
- App backgrounded during stream.

## Open Questions

- Should the first iOS bridge keep networking in Swift, or move networking into
  Rust sooner with platform-specific adapters?
- Should trusted LAN remain a supported first-class path, or should beta builds
  enforce Tailscale-first setup?
- What is the exact approval policy surface: one-shot approval only, scoped
  approvals, or reusable policies?
- Which notification provider setup is required for local development,
  TestFlight, and production?
- What server version negotiation contract should block unsupported app/server
  pairs?
- What should the first TestFlight bundle ID and signing profile be?

## Next Build Slice

The next highest-leverage slice is **M2 reducer retirement plus M3 real-device
diagnostics acceptance**. Audit remaining Swift-owned semantic state, move any
remaining product reducers into `jcode-mobile-core`, then run a physical iPhone
gateway loop that proves pair, connect, chat, stream, model state, reconnect,
reload recovery, and diagnostics against a live `jcode serve`.

Current local verification for the May 31 shared-core diagnostics slice:

- `cargo test -p jcode-mobile-core`
- `cargo test -p jcode-mobile-sim`
- `cargo check -p jcode-mobile-core -p jcode-mobile-sim`
- `swift run --package-path ios JCodeKitTests`
