---
name: verify-gate
description: Run jcode's full pre-merge verification — the canonical gate (scripts/refactor_phase1_verify.sh) plus the three CI-only ratcheting budgets and the dependency-boundary check that the gate omits — and interpret the result. Use before proposing or opening a PR, or before claiming a non-trivial change is safe to merge. NOT for fast iteration during development — use `cargo check -q` for that.
---

# verify-gate

jcode's local gate (`scripts/refactor_phase1_verify.sh`) does **not** run every check that CI runs. Three ratcheting budgets and the dependency-boundary check are **CI-only**, so a change can pass the local gate and still get a red CI run. This skill runs the gate *and* those extra checks so local verification matches CI.

Prefer delegating the actual run to the **`merge-gate-verifier`** subagent so it executes in a separate context (keeps verification independent from the context that authored the change, and keeps its long output out of the main thread).

## Steps

Run from the repo root. Collect all failures — do not stop at the first one.

1. **Canonical gate** (covers shadow check/build, `cargo check -q`, warning budget, code-size budget, `security_preflight.sh`, full tests, e2e, `clippy --all-targets --all-features -- -D warnings`):
   ```bash
   scripts/refactor_phase1_verify.sh
   ```
2. **CI-only budgets** (each must exit 0; each prints the offending files on failure):
   ```bash
   python3 scripts/check_panic_budget.py
   python3 scripts/check_swallowed_error_budget.py
   python3 scripts/check_test_size_budget.py
   ```
3. **CI-only dependency boundaries:**
   ```bash
   python3 scripts/check_dependency_boundaries.py
   ```

## Interpreting failures

- **warning / clippy** — `cargo clippy --fix` for autofixable lints; the gate is `-D warnings`, zero tolerance.
- **panic budget** — a new `.unwrap()/.expect()/panic!/todo!/unimplemented!` in production code. Propagate a `Result`, use `debug_assert!`, or handle the error.
- **swallowed-error budget** — new `let _ =`, `.ok()`, or `.unwrap_or_default()` that hides a failure. Log context or name the binding to show intent.
- **code-size / test-size budget** — a tracked file grew past 1200 LOC or a new oversized file appeared. Split it; prefer landing new code in the owning `crates/jcode-*` rather than root `src/`.
- **dependency boundaries** — a `*-types` crate gained a forbidden runtime dependency. See `docs/CRATE_OWNERSHIP_BOUNDARIES.md`.

## Intentional baseline changes

If a budget grew because of **intentional** cleanup or an accepted increase, the change owner (a human, or you only with explicit approval) may refresh the baseline with `scripts/<check>.py --update`. Never run `--update` as part of routine verification — that defeats the ratchet.

## Notes

- OOM during the gate's build? Don't retry locally — use `scripts/remote_build.sh` (see CLAUDE.md / AGENTS.md).
- This gate is slow by design. For the inner dev loop use `cargo check -q` or `scripts/test_fast.sh`.
