---
name: project-conventions
description: Background knowledge of jcode's non-obvious invariants — the iteration loop, crate-ownership/code-placement rules, the ratcheting budgets, the install/self-dev layout, and protected paths. Loaded so changes follow the project's discipline without re-reading every doc. Reference when deciding where code lands, how to iterate, or whether an action is safe.
user-invocable: false
---

# jcode project conventions

Standing rules for working in this repo. These are the invariants that bite repeatedly; the authoritative deep docs are linked inline.

## Iteration loop

- Iterate with `cargo check -q` — **never** `cargo build` for a fast feedback loop.
- Default features (`pdf`) are fine; pass `--no-default-features` if you don't need them. `--all-features` drags in the heavy `embeddings`/ONNX path (~163 crates) — only the merge gate needs it.
- Tests: prefer `cargo test -p <crate>` or `--test <name>` while iterating; reserve full `cargo test` for the gate. `scripts/test_fast.sh` runs lib+bins quickly.
- OOM during a local build? Don't retry locally — use `scripts/remote_build.sh`.

## Where code lands (crate ownership)

- The workspace splits by responsibility (~50 `jcode-*` crates). **Put new code in the owning `crates/jcode-*`, not root `src/`** — the code-size budget enforces this, and root `src/` is mid-decomposition.
- Keep the **type-vs-behavior split**: `*-types` / `*-core` crates hold DTOs/helpers only (serde/chrono/sibling types). Behavior, I/O, async, and network belong in runtime crates. `scripts/check_dependency_boundaries.py` blocks runtime deps in type crates.
- When moving a type out of root, leave a `pub use` facade to preserve the API. See `docs/CRATE_OWNERSHIP_BOUNDARIES.md` and `docs/MODULAR_ARCHITECTURE_RFC.md`. To create a crate, use the `new-crate` skill.

## Budgets are real, not advisory (and some are CI-only)

Five ratcheting budgets fail CI on regression: warning, panic (`.unwrap()/.expect()/panic!/...`), code-size (>1200 LOC), swallowed-error (`let _ =`/`.ok()`/`.unwrap_or_default()`), test-size. The local gate (`refactor_phase1_verify.sh`) runs only **warning + code-size**; panic, swallowed-error, test-size, and the dependency-boundary check are **CI-only**. Run the full set with the `verify-gate` skill before claiming a change is merge-safe. Interpret a failure with the `budget-fix` skill. Never run a budget's `--update` without explicit human approval — it moves the ratchet.

## Install / self-dev layout — do not fight it

- Launcher chain: `~/.local/bin/jcode → ~/.jcode/builds/current/jcode → versions/<hash>/jcode`. `~/.local/bin` must precede `~/.cargo/bin` on PATH, or a stale `cargo install`-ed jcode shadows the launcher.
- `scripts/install_release.sh` is the **only** blessed way to update the `current` launcher. Never hand-`cp` into `~/.jcode/builds/`.
- Installers never touch `~/.jcode/{config.toml,memory,skills}`; provider creds live in `~/.jcode/{auth,openai-auth,gemini_oauth,azure-auth}.json`.

## Protected paths (the PreToolUse hook enforces these)

Never directly Edit/Write: `~/.jcode/builds/**`, the auth `*.json` files, `~/.jcode/config.toml`, `~/.jcode/memory/**`, `~/.jcode/skills/**`, `~/.local/bin/jcode`, `target/**`. Use the scripts or the jcode CLI.

## Release

`Cargo.toml` version bump is part of release (patch-vs-minor per `AGENTS.md`); a pushed `v*` tag triggers the multi-arch build + Homebrew/AUR update. Use the `release-jcode` skill; never auto-bump or auto-tag.
