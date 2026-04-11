# CLAUDE.md

This file provides guidance to Claude Code when working with
this repository.

## Project

thurkube is a Kubernetes controller built with Rust and
kube-rs. It watches custom resources (`ClaudeCodeJob`) and
manages the lifecycle of Claude Code agent sessions running
as pods on a Kubernetes cluster — replacing the current Argo
Workflows-based orchestration in thurspace.

## Build & Development Commands

```bash
# Build
cargo build

# Build release
cargo build --release

# Run locally (requires kubeconfig)
cargo run
```

## Testing

```bash
# Run all tests
cargo nextest run --all

# Run a single test
cargo nextest run --all -E 'test(test_name)'
```

## Linting & Formatting

```bash
# Format check
cargo fmt --all -- --check

# Clippy
cargo clippy --all-targets --all-features -- -D warnings

# Dependency audit
cargo deny check advisories
cargo deny check bans licenses sources

# Documentation (warnings as errors)
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

## Docker

```bash
# Build image
docker build -t thurkube:dev .

# Build with version injection
docker build --build-arg THURKUBE_RELEASE_VERSION=v0.1.0 -t thurkube:0.1.0 .
```

## Conventional Commits

All commits must follow
[Conventional Commits](https://www.conventionalcommits.org/).
Enforced by pre-commit hooks.

- **Types**: feat, fix, perf, refactor, docs, style, test,
  chore, ci, build, revert
- **Scopes**: crd, controller, reconciler, api, config, docs,
  deps
