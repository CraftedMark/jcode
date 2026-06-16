#!/usr/bin/env bash
# Update this fork against upstream jcode and rebuild the local binary.
#
# What it does, in order:
#   1. Sanity-checks the git repo (clean tracked tree, detected remotes).
#   2. Backs up ~/.jcode/{config.toml,memory,skills} to ~/jcode-backups/.
#   3. Fetches the detected upstream remote (1jehuang/jcode).
#   4. Rebases the current branch onto upstream/master.
#   5. Pushes the rebased current branch to the detected fork remote unless --no-push.
#   6. Runs `scripts/install_release.sh --fast` to rebuild and update the
#      live binary at ~/.jcode/builds/current/jcode (and ~/.local/bin/jcode).
#
# Flags:
#   --dry-run    Show what would happen; don't fetch, rebase, push, or build.
#   --no-push    Skip pushing the rebased branch to your fork remote.
#   --no-build   Skip the rebuild/install step (just rebase).
#   --lto        Use release-lto profile (slower, smaller). Default: --fast.
#
# Remote detection:
#   upstream remote: URL containing 1jehuang/jcode, otherwise remote named upstream.
#   fork remote: branch pushRemote/remote.pushDefault, otherwise origin or another
#                non-upstream remote. Push is skipped if no fork is detected.
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
ok()    { printf "${c_green}OK${c_reset} %s\n" "$*"; }
warn()  { printf "${c_yellow}!${c_reset}  %s\n" "$*"; }
die()   { printf "${c_red}ERROR${c_reset} %s\n" "$*" >&2; exit 1; }
run()   {
  if $dry_run; then
    printf "${c_yellow}[dry-run]${c_reset}"
    printf " %q" "$@"
    printf "\n"
  else
    "$@"
  fi
}

remote_exists() {
  git remote get-url "$1" >/dev/null 2>&1
}

remote_urls() {
  git remote get-url --all "$1" 2>/dev/null || true
}

remote_has_url_fragment() {
  local remote="$1"
  local fragment="$2"
  remote_urls "$remote" | grep -Fq "$fragment"
}

detect_upstream_remote() {
  local remote

  while IFS= read -r remote; do
    if remote_has_url_fragment "$remote" "1jehuang/jcode"; then
      printf "%s\n" "$remote"
      return 0
    fi
  done < <(git remote)

  if remote_exists upstream; then
    printf "upstream\n"
    return 0
  fi

  return 1
}

detect_fork_remote() {
  local upstream_remote="$1"
  local branch="$2"
  local candidate=""
  local remote

  candidate="$(git config "branch.${branch}.pushRemote" || true)"
  if [[ -n "$candidate" && "$candidate" != "$upstream_remote" ]] && remote_exists "$candidate"; then
    printf "%s\n" "$candidate"
    return 0
  fi

  candidate="$(git config remote.pushDefault || true)"
  if [[ -n "$candidate" && "$candidate" != "$upstream_remote" ]] && remote_exists "$candidate"; then
    printf "%s\n" "$candidate"
    return 0
  fi

  if [[ "$upstream_remote" != "origin" ]] && remote_exists origin; then
    printf "origin\n"
    return 0
  fi

  if remote_exists fork && [[ "$upstream_remote" != "fork" ]]; then
    printf "fork\n"
    return 0
  fi

  while IFS= read -r remote; do
    if [[ "$remote" != "$upstream_remote" ]]; then
      printf "%s\n" "$remote"
      return 0
    fi
  done < <(git remote)

  return 1
}

require_clean_tracked_tree() {
  if ! git diff --quiet --ignore-submodules --; then
    git status --short --untracked-files=no
    die "Tracked files have unstaged changes. Commit or stash them first."
  fi

  if ! git diff --cached --quiet --ignore-submodules --; then
    git status --short --untracked-files=no
    die "Tracked files have staged changes. Commit or stash them first."
  fi
}

warn_about_untracked_files() {
  local untracked_count
  untracked_count="$(git ls-files --others --exclude-standard | wc -l | tr -d ' ')"

  if [[ "$untracked_count" != "0" ]]; then
    warn "Ignoring $untracked_count untracked file(s)/dir entry(s); tracked tree is what must be clean."
  fi
}

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
[[ -n "$repo_root" ]] || die "Not inside a git repo."
cd "$repo_root"

# --- sanity checks --------------------------------------------------------

info "Sanity checks"

current_branch="$(git branch --show-current)"
[[ -n "$current_branch" ]] || die "Detached HEAD is not supported. Check out a branch first."
ok "On branch $current_branch"

upstream_remote="$(detect_upstream_remote)" \
  || die "Could not detect upstream remote. Add a remote for 1jehuang/jcode or name it 'upstream'."
upstream_url="$(git remote get-url "$upstream_remote")"
upstream_branch="master"
upstream_ref="refs/remotes/${upstream_remote}/${upstream_branch}"
upstream_display="${upstream_remote}/${upstream_branch}"
ok "Upstream remote: $upstream_remote = $upstream_url"

if fork_remote="$(detect_fork_remote "$upstream_remote" "$current_branch")"; then
  fork_url="$(git remote get-url "$fork_remote")"
  ok "Fork remote: $fork_remote = $fork_url"
else
  warn "No fork remote detected; will skip push."
  do_push=false
fi

require_clean_tracked_tree
ok "Tracked working tree clean"
warn_about_untracked_files

# --- backup userdata ------------------------------------------------------

info "Backing up user data (~/.jcode/{config.toml,memory,skills})"
backup_dir="$HOME/jcode-backups"
ts="$(date +%Y%m%d-%H%M%S)"
backup_file="$backup_dir/jcode-userdata-${ts}.tar.gz"
if $dry_run; then
  warn "[dry-run] would create $backup_file"
else
  mkdir -p "$backup_dir"
  tar czf "$backup_file" -C "$HOME/.jcode" config.toml memory skills 2>/dev/null \
    || die "Backup failed"
  ok "Backup: $backup_file ($(du -h "$backup_file" | awk '{print $1}'))"
fi

# --- fetch upstream -------------------------------------------------------

info "Fetching $upstream_remote (upstream)"
run git fetch "$upstream_remote" --tags --prune

if ! git show-ref --verify --quiet "$upstream_ref"; then
  die "Missing $upstream_display locally. Run without --dry-run once to fetch it, or check the upstream remote."
fi

local_ahead="$(git rev-list --count "${upstream_ref}..HEAD")"
upstream_ahead="$(git rev-list --count "HEAD..${upstream_ref}")"
ok "$current_branch is $local_ahead ahead, $upstream_ahead behind $upstream_display"

if [[ "$upstream_ahead" == "0" ]]; then
  ok "Current branch already contains $upstream_display. Nothing to rebase."
  if $do_push && [[ "$local_ahead" != "0" ]]; then
    info "Pushing $current_branch to $fork_remote"
    run git push "$fork_remote" "${current_branch}:${current_branch}" --force-with-lease
    ok "Pushed"
  elif $do_push; then
    ok "No local commits to push."
  else
    warn "Skipping push (--no-push or no fork remote)"
  fi
  if $do_build; then
    info "Rebuilding anyway (use --no-build to skip)"
    if [[ -n "$profile_flag" ]]; then
      run "$repo_root/scripts/install_release.sh" "$profile_flag"
    else
      run "$repo_root/scripts/install_release.sh"
    fi
  fi
  exit 0
fi

# --- preview local commits to be rebased ----------------------------------

info "Local commits that will be rebased onto $upstream_display:"
if [[ "$local_ahead" == "0" ]]; then
  echo "   (none)"
else
  git log --oneline "${upstream_ref}..HEAD" | sed 's/^/   /'
fi
echo

# --- rebase ---------------------------------------------------------------

info "Rebasing $current_branch onto $upstream_display"
if $dry_run; then
  run git rebase "$upstream_display"
else
  if ! git rebase "$upstream_display"; then
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
  info "Pushing rebased $current_branch to $fork_remote (force-with-lease)"
  run git push "$fork_remote" "${current_branch}:${current_branch}" --force-with-lease
  ok "Pushed"
else
  warn "Skipping push (--no-push or no fork remote)"
fi

# --- build + install ------------------------------------------------------

if $do_build; then
  info "Rebuilding and installing the live binary"
  if [[ -n "$profile_flag" ]]; then
    run "$repo_root/scripts/install_release.sh" "$profile_flag"
  else
    run "$repo_root/scripts/install_release.sh"
  fi
  ok "Live binary updated"
  if ! $dry_run; then
    "$HOME/.local/bin/jcode" --version || true
  fi
else
  warn "Skipping rebuild (--no-build). Run scripts/install_release.sh manually when ready."
fi

echo
ok "All done. Backup: $backup_file"
