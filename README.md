# thurkube

Kubernetes controller for orchestrating Claude Code agents.

## Overview

thurkube replaces the Argo Workflows-based orchestration layer
with a dedicated Kubernetes controller. Define a single
`ClaudeCodeJob` custom resource and the controller manages
everything: scheduling, config injection, persistence, and
lifecycle.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [pre-commit](https://pre-commit.com/)
- [cargo-nextest](https://nexte.st/)
- [cargo-deny](https://embarkstudios.github.io/cargo-deny/)
- [cocogitto](https://docs.cocogitto.io/)
- [Docker](https://docs.docker.com/get-docker/) (for image
  builds)

## Getting Started

```bash
# Clone
git clone git@github.com:Thurbeen/thurkube.git
cd thurkube

# Install git hooks
pre-commit install

# Build
cargo build

# Run tests
cargo nextest run --all
```

## Container Image

```bash
docker pull ghcr.io/thurbeen/thurkube:latest
```

## License

[Apache-2.0](LICENSE)
