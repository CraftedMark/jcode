---
name: budget-fix
description: Diagnose and fix a failing jcode ratcheting budget (warning, panic, code-size, swallowed-error, test-size) or the dependency-boundary check. Use when a budget script reports a regression — locally or in CI — to identify the offending files/patterns and apply the right remediation, or to decide whether a baseline --update is warranted.
---

# budget-fix

jcode enforces ratcheting budgets that fail on regression. This skill turns a budget failure into a fix. Identify which budget tripped (the script names it and lists offending files), then apply the matching remediation. Confirm the fix with the `verify-gate` skill.

## By budget

- **warning** (`scripts/check_warning_budget.sh`, baseline `warning_budget.txt`) — a new compiler warning. Run `cargo clippy --fix` for autofixable lints; otherwise resolve the warning. The clippy gate is `-D warnings`, so zero new warnings is the target.

- **panic** (`scripts/check_panic_budget.py`) — a new `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` in production code (test code is excluded). Fix by propagating a `Result` (`?`), using `expect("invariant: …")` only where the invariant is genuinely guaranteed, `debug_assert!` for debug-only checks, or handling the `None`/`Err` explicitly. Report which file grew and by how much.

- **swallowed-error** (`scripts/check_swallowed_error_budget.py`) — a new `let _ =`, `.ok()`, or `.unwrap_or_default()` that hides a failure. Fix by logging context at the discard site, naming the binding to document intent, or handling the error. This is a broad guardrail — the goal is intentional, visible error handling, not zero discards.

- **code-size** (`scripts/check_code_size_budget.py`, threshold 1200 LOC) — a tracked file grew past the limit or a new oversized file appeared. Split the module; prefer extracting the concern into its owning `crates/jcode-*` (use the `new-crate` skill) rather than another root `src/` file.

- **test-size** (`scripts/check_test_size_budget.py`, 1200 LOC) — an oversized test file. Split into focused test modules or factor shared setup into a helper.

- **dependency-boundary** (`scripts/check_dependency_boundaries.py`) — a `*-types` crate gained a forbidden runtime dependency. Move the behavior needing that dep into a runtime crate; keep the type crate dependency-light. See `docs/CRATE_OWNERSHIP_BOUNDARIES.md`.

## When the increase is intentional

If the growth is a deliberate, reviewed change (e.g. an accepted new `expect` with a proven invariant, or a file that legitimately must grow), the **human owner** may refresh the baseline:

```bash
python3 scripts/<check>.py --update   # panic / swallowed-error / code-size / test-size
scripts/check_warning_budget.sh --update
```

Only do this with explicit human approval and a clear note in the commit explaining why the ratchet moved. Never `--update` to make a failing check pass during routine work — that silently erodes the guardrail.

## Verify

After fixing, re-run the specific script, then `verify-gate` for the full local==CI picture before merge.
