# CLAUDE.md

This file provides guidance to Claude Code when working with
this repository.

## Project

thurkube is a Kubernetes controller built with Rust and
kube-rs. It defines 8 CRDs under `thurkube.thurbeen.eu/v1alpha1`
(AgentJob, AgentRuntime, AgentAuth, AgentRole, AgentSkill,
McpServer, Repository, ClusterAccess) and manages the lifecycle
of AI agent sessions running as pods — replacing the current
Argo Workflows-based orchestration in thurspace.

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

# Architecture rules only
cargo test --test architecture_rules

# E2E tests (requires a running cluster, e.g. k3d)
cargo run -- --crd | kubectl apply -f -
cargo test --test e2e -- --ignored --nocapture

# Print all CRD YAMLs
cargo run -- --crd
```

### Test structure

- **Unit tests** — inline `#[cfg(test)]` in each `src/crd/*.rs`
- **Integration tests** — `tests/` directory:
  - `crd_schema.rs` — CRD YAML validity and schema checks
  - `architecture_rules.rs` — module isolation enforcement
  - `version_format.rs` — build.rs version string validation
- **E2E tests** — `tests/e2e.rs` (`#[ignore]`, needs a cluster):
  - Installs all 8 CRDs on a real cluster
  - Creates/deletes custom resources for each CRD
  - Full AgentJob with cross-references
  - CI runs these against a k3d cluster
- **Nextest config** — `.config/nextest.toml` (CI profile has
  retries + fail-fast)

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
- **Scopes**: crd, controller, reconciler, api, config, ci,
  docs, deps
