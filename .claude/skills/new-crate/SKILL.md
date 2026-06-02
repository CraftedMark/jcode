---
name: new-crate
description: Scaffold a new jcode-<name> workspace crate the way the crate-split refactor expects — create crates/jcode-<name> with a Cargo.toml that inherits workspace settings, register it in the root workspace members, wire it as a path dependency where it's consumed, and (when extracting code out of root src/) leave a pub use facade so the public API is preserved. Use when adding a focused crate or extracting a concern out of monolithic root src/.
---

# new-crate

The workspace splits by responsibility (~50 `jcode-*` crates), and the active refactor is moving code out of monolithic root `src/` into owning crates while keeping the API stable. This skill scaffolds a new crate that fits that direction. **Read `docs/CRATE_OWNERSHIP_BOUNDARIES.md` and the root `Cargo.toml` first** and mirror the existing conventions rather than inventing structure.

## Steps

1. **Confirm the concern and crate kind.** Is this a *types/contract* crate (`*-types` / `*-core`: only DTOs/helpers, deps limited to serde/chrono/sibling type crates) or a *runtime/behavior* crate? The kind dictates allowed dependencies (`scripts/check_dependency_boundaries.py` enforces that `*-types` crates stay free of heavy runtime deps).
2. **Create the crate** at `crates/jcode-<name>/`:
   - `Cargo.toml` — match a sibling crate exactly: use `[package]` with `name = "jcode-<name>"`, `version.workspace = true`, `edition.workspace = true` (and any other `*.workspace = true` keys the siblings use). Add only the dependencies the concern needs; for shared deps use `<dep>.workspace = true`.
   - `src/lib.rs` — the crate root.
3. **Register in the workspace.** Add `"crates/jcode-<name>"` to root `Cargo.toml` `[workspace] members` (match the existing list's formatting/ordering).
4. **Wire the path dependency** into each consuming crate's `Cargo.toml`: `jcode-<name> = { path = "../jcode-<name>" }` (or `{ path = "crates/jcode-<name>" }` from root). Only add it where actually used.
5. **If extracting from root `src/`** — move the type/module, then leave a facade in the original location so external callers don't break: `pub use jcode_<name>::Thing;`. Follow the Move Checklist in `docs/CRATE_OWNERSHIP_BOUNDARIES.md`.
6. **Verify**: `cargo check -q -p jcode-<name>` then `python3 scripts/check_dependency_boundaries.py`. For a full check before merge, use the `verify-gate` skill.

## Guardrails

- Land the concern in the new crate; don't grow root `src/` (the code-size budget enforces this).
- Keep `*-types` crates dependency-light — adding a runtime dep there fails the boundary check.
- Don't add the new crate as a dep to a contract crate it would invert (deps flow downward — see `docs/MODULAR_ARCHITECTURE_RFC.md`).
- Bump nothing in `~/.jcode/**`; this is source-tree only.
