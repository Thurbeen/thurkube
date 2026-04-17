# thurkube

A Kubernetes controller for orchestrating Claude Code agents
as native batch workloads.

[Website](https://thurbeen.github.io/thurkube/)
· [Documentation](https://thurbeen.github.io/thurkube/docs/)
· [Helm chart](charts/thurkube)
· [Container image](https://github.com/Thurbeen/thurkube/pkgs/container/thurkube)

## What it is

thurkube replaces the Argo Workflows-based agent orchestration
layer in [thurspace](https://github.com/Thurbeen/thurspace)
with a single, purpose-built operator written in Rust on top
of [kube-rs](https://github.com/kube-rs/kube). Authoring an
agent run becomes one custom resource — `AgentJob` — and the
controller takes care of scheduling, config injection, RBAC,
persistence, secret wiring, and lifecycle.

The same building blocks (runtimes, auth, roles, skills, MCP
servers, repositories, cluster access) are first-class CRDs
that can be reused across many `AgentJob`s.

## Architecture

A single binary runs an event-driven reconciler against eight
namespaced CRDs in the `thurkube.thurbeen.eu/v1alpha1` API
group. The controller uses server-side apply with field
manager `thurkube`, owns its child resources (Jobs, CronJobs,
ConfigMaps, ServiceAccounts, PVCs), and exposes
`/healthz` + `/readyz` on `:8080` for Kubernetes probes.

```text
┌────────────────────┐    watches    ┌──────────────────┐
│      AgentJob      │──────────────►│  thurkube ctrl   │
└─────────┬──────────┘               └────────┬─────────┘
          │ resolves                          │ owns
          ▼                                   ▼
┌──────────────────────────┐         ┌────────────────────┐
│ AgentRuntime  AgentAuth  │         │  Job / CronJob     │
│ AgentRole     AgentSkill │         │  ConfigMap         │
│ McpServer     Repository │         │  ServiceAccount    │
│ ClusterAccess            │         │  PVC               │
└──────────────────────────┘         └────────────────────┘
```

### Custom resources

| Kind             | Short  | Purpose                                              |
| ---------------- | ------ | ---------------------------------------------------- |
| `AgentJob`       | `aj`   | Scheduled or one-shot agent run; the orchestration unit. |
| `AgentRuntime`   | `ar`   | Container image, entrypoint, mount conventions.      |
| `AgentAuth`      | `aa`   | Reference to a Secret holding the agent auth token.  |
| `AgentRole`      | `arl`  | Allowed-tools list for the agent.                    |
| `AgentSkill`     | `ask`  | Reusable skill from a GitHub repository.             |
| `McpServer`      | `mcp`  | Local command or remote URL for an MCP server.       |
| `Repository`     | `repo` | GitHub repo + token reference to clone.              |
| `ClusterAccess`  | `ca`   | RBAC rules — controller materializes SA + ClusterRole + Binding. |

`AgentJob` is the primary resource. It references the others
by name in the same namespace. With a `schedule` field set,
the controller emits a `CronJob`; without one, a one-shot
`Job`. A `configHash` in the status field detects ConfigMap
drift and triggers redeployment.

See the [Architecture](https://thurbeen.github.io/thurkube/docs/architecture.html)
page for the deeper design and the
[CRD Reference](https://thurbeen.github.io/thurkube/docs/crd-reference.html)
for the full field listing.

## Prerequisites

| Tool                                              | Required for                  |
| ------------------------------------------------- | ----------------------------- |
| Kubernetes 1.28+                                  | running thurkube              |
| [Helm](https://helm.sh/) 3.16+                    | installing the chart          |
| [Rust](https://rustup.rs/) (stable, 1.75+)        | building from source          |
| [cargo-nextest](https://nexte.st/)                | running the test suite        |
| [cargo-deny](https://embarkstudios.github.io/cargo-deny/) | dependency audit      |
| [pre-commit](https://pre-commit.com/)             | git hooks                     |
| [cocogitto](https://docs.cocogitto.io/)           | conventional commit linting   |
| [Docker](https://docs.docker.com/get-docker/)     | building the container image  |
| [k3d](https://k3d.io/) or kind                    | running the E2E suite locally |

## Install with Helm

```bash
helm install thurkube oci://ghcr.io/thurbeen/charts/thurkube \
  --namespace thurkube-system --create-namespace
```

By default the chart installs the eight CRDs and runs a single
controller replica with locked-down pod security
(`runAsNonRoot`, `readOnlyRootFilesystem`, dropped
capabilities, RuntimeDefault seccomp). Override anything via
`--set` or `--values`:

```bash
helm install thurkube oci://ghcr.io/thurbeen/charts/thurkube \
  --namespace thurkube-system --create-namespace \
  --set image.tag=v0.1.0 \
  --set logLevel=debug \
  --set crds.install=false
```

To install the CRDs separately (recommended for clusters where
the controller and CRDs have different lifecycles):

```bash
docker run --rm ghcr.io/thurbeen/thurkube:latest --crd \
  | kubectl apply -f -
```

## Quickstart

A minimal agent that runs every six hours, fixing failing PRs
on a single repo:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: claude-code-secrets
  namespace: agents
stringData:
  CLAUDE_CODE_OAUTH_TOKEN: "<your-claude-token>"
  GH_TOKEN: "<your-github-token>"
---
apiVersion: thurkube.thurbeen.eu/v1alpha1
kind: AgentRuntime
metadata: { name: claude-code, namespace: agents }
spec:
  image: ghcr.io/thurbeen/claude-code-job:latest
  authEnvVar: CLAUDE_CODE_OAUTH_TOKEN
  configPath: /etc/claude-code-job
  persistPath: /var/lib/claude-code-job
---
apiVersion: thurkube.thurbeen.eu/v1alpha1
kind: AgentAuth
metadata: { name: claude-oauth, namespace: agents }
spec:
  secretRef: { name: claude-code-secrets, key: CLAUDE_CODE_OAUTH_TOKEN }
---
apiVersion: thurkube.thurbeen.eu/v1alpha1
kind: AgentRole
metadata: { name: default, namespace: agents }
spec:
  allowedTools: [Bash, Read, Edit, Write, Glob, Grep]
---
apiVersion: thurkube.thurbeen.eu/v1alpha1
kind: Repository
metadata: { name: thurkube, namespace: agents }
spec:
  owner: Thurbeen
  name: thurkube
  tokenSecretRef: { name: claude-code-secrets, key: GH_TOKEN }
---
apiVersion: thurkube.thurbeen.eu/v1alpha1
kind: AgentJob
metadata: { name: pr-fixer, namespace: agents }
spec:
  schedule: "0 */6 * * *"
  timezone: Europe/Paris
  runtimeRef: claude-code
  authRef: claude-oauth
  roleRef: default
  repositoryRefs: [thurkube]
  prompt: "Fix any failing PRs on this repository."
  persist: true
```

```bash
kubectl apply -f agent.yaml
kubectl get aj -n agents -o wide
kubectl describe aj pr-fixer -n agents
```

## Development

```bash
# Clone
git clone git@github.com:Thurbeen/thurkube.git
cd thurkube

# Install git hooks
pre-commit install

# Build
cargo build

# Run unit + integration tests
cargo nextest run --all

# Architecture rules only
cargo test --test architecture_rules

# Print all CRD YAMLs
cargo run -- --crd

# Run E2E tests against a k3d cluster
k3d cluster create thurkube-dev
cargo run -- --crd | kubectl apply -f -
cargo test --test e2e -- --ignored --nocapture
```

### Linting & audit

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo deny check advisories
cargo deny check bans licenses sources
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
helm lint charts/thurkube
```

### Container image

```bash
# Local build
docker build -t thurkube:dev .

# With version injection
docker build --build-arg THURKUBE_RELEASE_VERSION=v0.1.0 \
  -t thurkube:0.1.0 .

# Pre-built
docker pull ghcr.io/thurbeen/thurkube:latest
```

## Contributing

- All commits follow
  [Conventional Commits](https://www.conventionalcommits.org/)
  and are verified by cocogitto in CI.
- Allowed types: `feat`, `fix`, `perf`, `refactor`, `docs`,
  `style`, `test`, `chore`, `ci`, `build`, `revert`.
- Allowed scopes: `crd`, `controller`, `reconciler`, `api`,
  `config`, `ci`, `docs`, `deps`.
- Run `pre-commit install` after cloning. The hook chain
  enforces formatting, clippy, commit message style, and the
  full `cargo nextest` suite.
- New CRD fields require a unit test in
  `src/crd/<resource>.rs` and a corresponding entry in the
  E2E suite (`tests/e2e.rs`).
- Module isolation rules are enforced by
  `tests/architecture_rules.rs` — read it before adding new
  modules.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
