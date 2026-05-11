#!/usr/bin/env bash
# Update this fork against upstream jcode and rebuild the local binary.
#
# What it does, in order:
#   1. Sanity-checks the git repo (clean tree, on master, expected remotes).
#   2. Backs up ~/.jcode/{config.toml,memory,skills} to ~/jcode-backups/.
#   3. Fetches `origin` (upstream: 1jehuang/jcode).
#   4. Rebases your local-only commits onto `origin/master`.
#   5. Pushes the rebased branch to `fork` (your remote) unless --no-push.
#   6. Runs `scripts/install_release.sh --fast` to rebuild and update the
#      live binary at ~/.jcode/builds/current/jcode (and ~/.local/bin/jcode).
#
# Flags:
#   --dry-run    Show what would happen; don't fetch, rebase, push, or build.
#   --no-push    Skip pushing the rebased branch to `fork`.
#   --no-build   Skip the rebuild/install step (just rebase).
#   --lto        Use release-lto profile (slower, smaller). Default: --fast.
#
# Conventions assumed (matches this repo today):
#   origin -> upstream (1jehuang/jcode)
#   fork   -> your remote (CraftedMark/jcode or similar)
#   working branch: master
set -euo pipefail

dry_run=false
do_push=true
do_build=true
profile_flag="--fast"

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=true; shift ;;
    --no-push) do_push=false; shift ;;
    --no-build) do_build=false; shift ;;
    --lto) profile_flag=""; shift ;;
    -h|--help)
      sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "Unknown flag: $1" >&2; exit 1 ;;
  esac
done

# --- helpers --------------------------------------------------------------

c_blue='\033[1;34m'; c_green='\033[1;32m'; c_yellow='\033[1;33m'; c_red='\033[1;31m'; c_reset='\033[0m'
info()  { printf "${c_blue}==>${c_reset} %s\n" "$*"; }
ok()    { printf "${c_green}✔${c_reset}  %s\n" "$*"; }
warn()  { printf "${c_yellow}!${c_reset}  %s\n" "$*"; }
die()   { printf "${c_red}✖${c_reset}  %s\n" "$*" >&2; exit 1; }
run()   {
  if $dry_run; then
    printf "${c_yellow}[dry-run]${c_reset} %s\n" "$*"
  else
    eval "$@"
  fi
}

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
[[ -n "$repo_root" ]] || die "Not inside a git repo."
cd "$repo_root"

# --- sanity checks --------------------------------------------------------

info "Sanity checks"

current_branch="$(git branch --show-current)"
[[ "$current_branch" == "master" ]] \
  || die "Expected to be on branch 'master', got '$current_branch'. Switch first."
ok "On branch master"

git remote get-url origin >/dev/null 2>&1 \
  || die "No 'origin' remote configured (should be 1jehuang/jcode upstream)."
origin_url="$(git remote get-url origin)"
ok "origin = $origin_url"

if git remote get-url fork >/dev/null 2>&1; then
  fork_url="$(git remote get-url fork)"
  ok "fork   = $fork_url"
else
  warn "No 'fork' remote configured — will skip push."
  do_push=false
fi

if [[ -n "$(git status --porcelain)" ]]; then
  # Untracked .DS_Store files litter this repo; ignore them but block real changes.
  dirty="$(git status --porcelain | grep -v '\.DS_Store$' || true)"
  if [[ -n "$dirty" ]]; then
    echo "$dirty"
    die "Working tree has uncommitted changes (excluding .DS_Store). Commit or stash first."
  fi
  warn "Untracked .DS_Store files present; ignoring."
fi
ok "Working tree clean"

# --- backup userdata ------------------------------------------------------

info "Backing up user data (~/.jcode/{config.toml,memory,skills})"
backup_dir="$HOME/jcode-backups"
mkdir -p "$backup_dir"
ts="$(date +%Y%m%d-%H%M%S)"
backup_file="$backup_dir/jcode-userdata-${ts}.tar.gz"
if $dry_run; then
  warn "[dry-run] would create $backup_file"
else
  tar czf "$backup_file" -C "$HOME/.jcode" config.toml memory skills 2>/dev/null \
    || die "Backup failed"
  ok "Backup: $backup_file ($(du -h "$backup_file" | awk '{print $1}'))"
fi

# --- fetch upstream -------------------------------------------------------

info "Fetching origin (upstream)"
run "git fetch origin --tags --prune"

local_ahead="$(git rev-list --count master ^origin/master)"
upstream_ahead="$(git rev-list --count origin/master ^master)"
ok "Local is $local_ahead ahead, $upstream_ahead behind origin/master"

if [[ "$upstream_ahead" == "0" ]]; then
  ok "Already up to date with upstream. Nothing to rebase."
  if $do_build; then
    info "Rebuilding anyway (use --no-build to skip)"
    run "\"$repo_root/scripts/install_release.sh\" $profile_flag"
  fi
  exit 0
fi

# --- preview local commits to be rebased ----------------------------------

info "Local commits that will be rebased onto origin/master:"
git log --oneline origin/master..master | sed 's/^/   /'
echo

# --- rebase ---------------------------------------------------------------

info "Rebasing master onto origin/master"
if $dry_run; then
  warn "[dry-run] would: git rebase origin/master"
else
  if ! git rebase origin/master; then
    warn "Rebase hit conflicts."
    cat <<EOF

   Resolve conflicts in the listed files, then:
     git add <file>...
     git rebase --continue

   To abort and restore the previous state:
     git rebase --abort

   Your binary is unchanged; userdata backup is at:
     $backup_file
EOF
    exit 1
  fi
  ok "Rebase complete"
fi

# --- push -----------------------------------------------------------------

if $do_push; then
  info "Pushing rebased master to fork (force-with-lease)"
  run "git push fork master --force-with-lease"
  ok "Pushed"
else
  warn "Skipping push (--no-push or no fork remote)"
fi

# --- build + install ------------------------------------------------------

if $do_build; then
  info "Rebuilding and installing the live binary"
  run "\"$repo_root/scripts/install_release.sh\" $profile_flag"
  ok "Live binary updated"
  if ! $dry_run; then
    "$HOME/.local/bin/jcode" --version || true
  fi
else
  warn "Skipping rebuild (--no-build). Run scripts/install_release.sh manually when ready."
fi

echo
ok "All done. Backup: $backup_file"
