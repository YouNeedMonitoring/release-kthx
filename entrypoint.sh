#!/usr/bin/env bash
set -euo pipefail

# GitHub Actions preserves hyphens in INPUT_ env vars (INPUT_DRY-RUN, not
# INPUT_DRY_RUN). Hyphens are invalid in POSIX variable names, so we must
# use printenv to read them.
mode="$(printenv 'INPUT_MODE' 2>/dev/null || echo "plan")"
path="$(printenv 'INPUT_PATH' 2>/dev/null || echo ".")"
from_tag="$(printenv 'INPUT_FROM-TAG' 2>/dev/null || echo "")"
base_branch="$(printenv 'INPUT_BASE-BRANCH' 2>/dev/null || echo "main")"
pr_branch="$(printenv 'INPUT_PR-BRANCH' 2>/dev/null || echo "release-kthx/release-pr")"
dry_run="$(printenv 'INPUT_DRY-RUN' 2>/dev/null || echo "true")"
push="$(printenv 'INPUT_PUSH' 2>/dev/null || echo "false")"
force="$(printenv 'INPUT_FORCE' 2>/dev/null || echo "false")"

configure_safe_directory() {
  local repo_path="$1"
  local abs_path

  if [[ "$repo_path" = /* ]]; then
    abs_path="$repo_path"
  else
    abs_path="$(pwd)/$repo_path"
  fi

  if command -v realpath >/dev/null 2>&1; then
    abs_path="$(realpath -m "$abs_path")"
  fi

  export HOME="${HOME:-/github/home}"
  mkdir -p "$HOME"

  git config --global --add safe.directory "$abs_path"

  if [[ -d /github/workspace ]]; then
    git config --global --add safe.directory /github/workspace
  fi
}

configure_safe_directory "$path"

case "$mode" in
  init)
    args=(init --path "$path")
    if [[ "$force" == "true" ]]; then
      args+=(--force)
    fi
    ;;
  check)
    args=(check --path "$path")
    ;;
  plan)
    args=(plan --path "$path")
    if [[ -n "$from_tag" ]]; then
      args+=(--from-tag "$from_tag")
    fi
    ;;
  release-pr)
    args=(release-pr --path "$path" --base-branch "$base_branch" --pr-branch "$pr_branch")
    if [[ -n "$from_tag" ]]; then
      args+=(--from-tag "$from_tag")
    fi
    ;;
  release)
    args=(release --path "$path")
    if [[ -n "$from_tag" ]]; then
      args+=(--from-tag "$from_tag")
    fi
    if [[ "$dry_run" == "true" ]]; then
      args+=(--dry-run)
    fi
    if [[ "$push" == "true" ]]; then
      args+=(--push)
    fi
    ;;
  publish)
    args=(publish --path "$path")
    if [[ "$dry_run" == "true" ]]; then
      args+=(--dry-run)
    fi
    if [[ "$push" == "true" ]]; then
      args+=(--push)
    fi
    ;;
  publish-on-merge)
    args=(publish-on-merge --path "$path")
    if [[ "$dry_run" == "true" ]]; then
      args+=(--dry-run)
    fi
    if [[ "$push" == "true" ]]; then
      args+=(--push)
    fi
    ;;
  *)
    echo "unsupported mode: $mode" >&2
    echo "valid modes: init | check | plan | release-pr | release | publish | publish-on-merge" >&2
    exit 2
    ;;
esac

exec /usr/local/bin/release-kthx "${args[@]}"
