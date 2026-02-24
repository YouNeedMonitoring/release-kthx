# release-kthx

`release-kthx` is a release-plz-style automation tool designed primarily for private repositories.

It ships first as a GitHub Action (Docker action), with a Rust CLI runtime inside the action image.

## Why private-first

- No crates.io dependency in the default flow
- Conventional-commit based version planning from git history
- Works with org-private repos and custom GitHub tokens in CI
- Generates changelog text and creates annotated git tags for releases

## GitHub Action usage

```yaml
name: release

on:
  workflow_dispatch:
  push:
    branches: [main]

jobs:
  release-kthx:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Plan release
        uses: ./. # replace with your published action ref
        with:
          mode: plan
          path: .

      - name: Create tag (example)
        uses: ./. # replace with your published action ref
        with:
          mode: release
          path: .
          dry-run: "false"
          push: "true"
```

## Action inputs

- `mode`: `init` | `check` | `plan` | `release` (default `plan`)
- `path`: repository path (default `.`)
- `from-tag`: optional starting tag
- `dry-run`: only for `release` (default `true`)
- `push`: only for `release`; pushes created tag to origin (default `false`)
- `force`: only for `init` (default `false`)

## Local CLI usage

```bash
cargo run -- init --path .
cargo run -- check --path .
cargo run -- plan --path .
cargo run -- release --path . --dry-run
```

## Config file

`release-kthx.toml`:

```toml
[release]
tag_template = "v{{ version }}"

[github]
create_release = true
token_env = "GITHUB_TOKEN"
repository_env = "GITHUB_REPOSITORY"
```

## This repository uses itself

This repo includes `release-kthx.toml` and `.github/workflows/self-release.yml` so the action validates and plans releases for itself on every push/PR, and can cut tags via manual dispatch.

## Workspace layout

- `release-kthx` (root crate): application/infrastructure layer (CLI, git adapter, config, action wiring)
- `crates/release-kthx-domain`: domain layer with release planning model and versioning rules
