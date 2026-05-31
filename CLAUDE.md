# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

`jcode` is a Rust 2024-edition Cargo **workspace** (~50 crates) that builds a TUI coding-agent harness. The root `jcode` crate at `src/` is both a `[lib]` and the primary `[[bin]]`; the bulk of functionality lives in `crates/jcode-*`. Heavy subsystems are gated behind features (`pdf` is on by default; `embeddings` is opt-in because it pulls ONNX/tokenizers, ~163 extra crates).

**This binary self-modifies.** `jcode` can enter "self-dev mode" where it edits, builds, installs, and reloads its own source — keep that in mind before treating `~/.jcode/builds/*` as throwaway state. There is also `[profile.selfdev]` in `Cargo.toml` specifically for in-process rebuild loops.

Authoritative companion docs (read these before non-trivial work in their area):
- `AGENTS.md` — fork-sync, install paths, logs, debug socket, release/version bump rules.
- `docs/REFACTORING.md` + `scripts/refactor_phase1_verify.sh` — the non-negotiable verification gate.
- `docs/MEMORY_ARCHITECTURE.md`, `docs/SWARM_ARCHITECTURE.md`, `docs/SERVER_ARCHITECTURE.md`, `docs/AMBIENT_MODE.md`, `docs/SAFETY_SYSTEM.md`, `docs/BROWSER_PROVIDER_PROTOCOL.md` — per-subsystem deep dives.
- `docs/CRATE_OWNERSHIP_BOUNDARIES.md` + `docs/MODULAR_ARCHITECTURE_RFC.md` + `docs/COMPILE_PERFORMANCE_PLAN.md` — what belongs where, and the crate-split roadmap.

## Commands

Iterate with `cargo check -q` — never `cargo build` for a fast feedback loop. Default features (`pdf`) are usually fine; pass `--no-default-features` if you don't need them.

| Goal | Command |
|---|---|
| Fast iteration | `cargo check -q` |
| Fast iteration with linker/sccache picks (Linux x86_64) | `scripts/dev_cargo.sh check` |
| Show what `dev_cargo.sh` will use (sccache, linker, profile) | `scripts/dev_cargo.sh --print-setup` |
| Full test run | `cargo test -q` |
| One crate's tests | `cargo test -p <jcode-crate-name> -q` |
| Single test | `cargo test -q <test_name_or_module::path>` — append ` -- --nocapture` for stdout |
| E2E tests only | `cargo test --test e2e -q` (also `--test auth_login_flow`, `--test provider_matrix`) |
| Lib + bins fast loop + startup-budget check | `scripts/test_fast.sh` |
| Full e2e suite | `scripts/test_e2e.sh` |
| Lint (must be clean — gate is `-D warnings`) | `cargo clippy --all-targets --all-features -- -D warnings` |
| Pre-merge gate (the canonical "is this safe" check) | `scripts/refactor_phase1_verify.sh` |
| Release build (fast, no LTO) | `cargo build --profile release` |
| Release build (LTO, for distribution) | `cargo build --profile release-lto` |
| Install built release into launcher | `scripts/install_release.sh` (default LTO) or `scripts/install_release.sh --fast` |
| Heavy build on another machine | `scripts/remote_build.sh [--release] [-- test args]` (requires `JCODE_REMOTE_HOST` set, e.g. via `~/.config/jcode/remote-build.env`) |
| Same, but transparently through `dev_cargo.sh` | `JCODE_REMOTE_CARGO=1 scripts/dev_cargo.sh build --release` |
| Feature presets via env (avoid passing `--features` directly) | `JCODE_DEV_FEATURE_PROFILE=minimal\|pdf\|embeddings\|full scripts/dev_cargo.sh check` |

If a local build is OOM-killed (the machine ran out of resources for rustc/LTO), don't retry locally — use `scripts/remote_build.sh`. Self-dev/installed-binary debug builds go through `[profile.selfdev]` which `dev_cargo.sh` auto-detects and clamps for low-memory hosts (controlled by `JCODE_SELFDEV_LOW_MEMORY`).

## Install / runtime layout (don't fight this)

The launcher is a symlink chain. Touching the wrong link breaks updates and self-dev:

```
~/.local/bin/jcode          → launcher (must be BEFORE ~/.cargo/bin on PATH)
  └─ ~/.jcode/builds/current/jcode   → active local/source-built channel
       └─ ~/.jcode/builds/versions/<hash>/jcode   (immutable)
~/.jcode/builds/stable/jcode  → stable release channel
~/.jcode/builds/canary/jcode  → canary channel (still exists, not primary)
~/.jcode/logs/                → daily log files (jcode-YYYY-MM-DD.log)
~/.jcode/config.toml          → user config — installers never touch this
~/.jcode/memory/              → user memory store — installers never touch this
~/.jcode/skills/              → user skills — installers never touch this
~/.jcode/auth.json, ~/.jcode/openai-auth.json, ~/.jcode/gemini_oauth.json   → provider creds
```

Windows equivalents under `%LOCALAPPDATA%\jcode\…` — see `AGENTS.md`.

`scripts/install_release.sh` is the only blessed path to update the `current` launcher from a source build — it handles versioning, immutable storage, and the stable/current symlinks atomically. Do not hand-`cp` binaries into `~/.jcode/builds/`.

## Workspace shape (where things live)

The root `Cargo.toml` defines the workspace and pulls in each `crates/jcode-*` as a path dependency. The split is by responsibility, not by layer — pick the crate that owns the concern rather than dumping code into root `src/`:

- **Core runtime** — `jcode-agent-runtime`, `jcode-core`, `jcode-storage`, `jcode-session-types`, `jcode-message-types`, `jcode-protocol`, `jcode-gateway-types`
- **Memory + ambient + swarm** — `jcode-memory-types`, `jcode-compaction-core`, `jcode-ambient-types`, `jcode-overnight-core`, `jcode-swarm-core`, `jcode-embedding` (heavy ONNX, behind `embeddings` feature)
- **Providers** (one crate per auth surface) — `jcode-provider-core`, `jcode-provider-metadata`, `jcode-provider-openai`, `jcode-provider-openrouter`, `jcode-provider-gemini`, `jcode-azure-auth`, `jcode-auth-types`
- **Tools** — `jcode-tool-core`, `jcode-tool-types`
- **TUI surface** — `jcode-tui-core`, `jcode-tui-render`, `jcode-tui-style`, `jcode-tui-markdown`, `jcode-tui-mermaid`, `jcode-tui-messages`, `jcode-tui-tool-display`, `jcode-tui-usage-overlay`, `jcode-tui-account-picker`, `jcode-tui-session-picker`, `jcode-tui-workspace`
- **Lifecycle / infra** — `jcode-update-core`, `jcode-terminal-launch`, `jcode-build-support`, `jcode-config-types`, `jcode-notify-email`, `jcode-selfdev-types`, `jcode-side-panel-types`, `jcode-task-types`, `jcode-batch-types`, `jcode-background-types`, `jcode-usage-types`, `jcode-import-core`, `jcode-plan`
- **Mobile / desktop** — `jcode-mobile-core`, `jcode-mobile-ffi`, `jcode-mobile-sim`, `jcode-desktop`
- **Optional / heavy** — `jcode-pdf` (feature `pdf`, default on), `jcode-embedding` (feature `embeddings`, off)

Root `src/` contains the binaries (`main.rs`, `bin/harness.rs`, `bin/test_api.rs`, plus dev-only bins gated by `dev-bins`) and the still-monolithic `server.rs`, `session.rs`, `agent.rs`, `memory.rs`, `compaction.rs`, `telemetry.rs`, `update.rs`, etc. The active refactor (see `docs/REFACTORING.md` Phase 2+) is moving these into focused modules — when adding new code, prefer landing it in the right `crates/jcode-*` rather than enlarging the root files.

## What hurts when you skip it

- **Run the `scripts/refactor_phase1_verify.sh` gate before merging anything non-trivial.** It runs the shadow-env check, `check`, warning-budget guard, code-size-budget, security preflight, full tests, e2e, and `clippy -D warnings`. Skipping it is how regressions land.
- **Warning count is budgeted.** `scripts/check_warning_budget.sh` (and `warning_budget.txt`) will fail CI if you add a warning. Same idea for `check_panic_budget.py`, `check_code_size_budget.py`, `check_swallowed_error_budget.py`, `check_test_size_budget.py` — these are real budgets, not advisory.
- **`PATH` order matters.** `~/.local/bin` must come before `~/.cargo/bin`, otherwise a stale `cargo install`-installed `jcode` shadows the launcher and you'll debug a binary that isn't the one you just built.
- **`Cargo.toml` version bumps are part of release.** When cutting a release, decide patch vs. minor from the changes since the last release (see `AGENTS.md`).
- **Don't hand-edit `target/`, `~/.jcode/builds/`, or auth files.** Use the scripts.
- **Tests can be slow.** Prefer `cargo test -p <crate>` or a specific `--test <name>` while iterating; reserve `cargo test` (everything) for the verify gate.
