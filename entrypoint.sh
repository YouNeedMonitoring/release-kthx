#!/usr/bin/env bash
set -euo pipefail

mode="${INPUT_MODE:-plan}"
path="${INPUT_PATH:-.}"
from_tag="${INPUT_FROM_TAG:-}"
dry_run="${INPUT_DRY_RUN:-true}"
push="${INPUT_PUSH:-false}"
force="${INPUT_FORCE:-false}"

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
  *)
    echo "unsupported mode: $mode" >&2
    echo "valid modes: init | check | plan | release" >&2
    exit 2
    ;;
esac

exec /usr/local/bin/release-kthx "${args[@]}"
