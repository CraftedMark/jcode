---
name: release-jcode
description: Cut a jcode release — verify a clean tree, decide patch vs minor per AGENTS.md, bump the Cargo.toml version, run the full verify gate, commit, tag v<version>, and (locally) install via scripts/install_release.sh. The git tag push triggers .github/workflows/release.yml (multi-arch build + Homebrew/AUR update). User-invoked only; never auto-run.
disable-model-invocation: true
---

# release-jcode

Cutting a release is a side-effecting, partly irreversible flow (a pushed tag triggers a public multi-arch build and updates Homebrew/AUR). Run it only when the user explicitly asks. **Stop and confirm with the user** before the two irreversible steps: the tag push and `install_release.sh`.

## Preconditions

- On the intended release branch with a **clean working tree** (`git status` clean). If dirty, stop and report.
- The version source of truth is the workspace `Cargo.toml`. The launcher updates only through `scripts/install_release.sh` — never hand-`cp` a binary into `~/.jcode/builds/`.

## Steps

1. **Decide the bump.** Review changes since the last release tag (`git log $(git describe --tags --abbrev=0)..HEAD --oneline`). Apply the patch-vs-minor rule from `AGENTS.md`. Propose the new version to the user and get agreement before editing.
2. **Bump the version** in `Cargo.toml` (`[workspace.package]` / `[package]` `version`). Keep it a single, reviewable edit.
3. **Verify** — run the full gate via the `verify-gate` skill (or the `merge-gate-verifier` subagent). Do not proceed on any failure.
4. **Commit** the version bump with a release-style message (match the repo's existing release-commit convention; check `git log` for the format).
5. **Tag** — create `v<version>` (annotated). **Confirm with the user before pushing the tag**, since the push triggers `release.yml`.
6. **Install locally** (optional, on request) — `scripts/install_release.sh` (LTO) or `scripts/install_release.sh --fast`. This is the only blessed path to update the `current` launcher.

## Hard rules

- Never auto-bump the version or push a tag without explicit user approval at the moment of release.
- Never edit `~/.jcode/builds/**`, `~/.local/bin/jcode`, or auth files directly — the protect-paths hook will block these anyway; use the scripts.
- If the gate fails, the release stops. No exceptions.
- Confirm OS/arch coverage expectations match `release.yml` before announcing the release is "done"; the tag push only *starts* the build.
