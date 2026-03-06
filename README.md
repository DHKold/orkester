# Orkester

## Project Structure
Orkester is organized as a Rust cargo workspace to support modular development and extensibility. The main components are:

- **crates/core/**: The core application logic and main binary.
- **crates/common/**: Shared types, traits, and utilities used across the project.
- **crates/plugin-auth/**: Pluggable authentication providers (e.g., Password, LDAP, OIDC).
- **crates/plugin-authz/**: Pluggable authorization providers (e.g., OPA, RBAC).
- **crates/plugin-registry/**: WorkflowRegistry plugins (e.g., REST, S3, Git).
- **crates/plugin-persistence/**: Persistence plugins (e.g., SQL, file, custom backends).

Each crate is developed and tested independently, but all are managed together in the workspace for unified building, dependency management, and testing.

## Overview
Orkester is an ultra-fast, resilient, and secure workflow platform for orchestrating complex task execution across diverse environments. It provides a robust foundation for building, managing, and monitoring workflows (Works) composed of interdependent Tasks, supporting artifact flow, advanced scheduling, and secure API-driven management.

## Core Concepts
- **Task**: The atomic unit of execution. Tasks can run shell commands, scripts, containers (Kubernetes, Podman), or other actions. Tasks may produce or consume Artifacts, including special types such as Secrets.
- **Work**: A directed set of Tasks with defined dependencies and artifact flow. Works enable complex workflows, supporting conditional execution and parallelism.
- **Workspace**: A logical grouping of multiple Works. Workspaces provide isolation and multi-tenancy.
- **Artifact**: Data or files produced and/or consumed by Tasks, enabling data flow within and across Works. Special artifact types (e.g., Secrets) are supported.

## Architecture
- **REST API**: Comprehensive, versioned API for managing all entities. All operations (creation, update, execution, monitoring) are API-driven and documented (OpenAPI/Swagger).
- **Authentication Providers**: Pluggable modules for user authentication (Password, LDAP, OIDC, etc.).
- **Authorization Providers**: Pluggable modules for access control and RBAC (e.g., OPA integration), supporting multi-tenancy and fine-grained permissions.
- **WorkflowRegistry**: Pluggable registry for managing Works/Tasks, with default implementations for REST and S3, and support for custom plugins (e.g., Git, database).
- **Persistence Layer**: Pluggable persistence for all platform data (Workspaces, Works, Tasks, Execution State, History, Logs, Metrics, etc.), with default SQL and file-based options, and support for custom plugins.
- **Task Execution Engine**: Abstracts execution backends (local shell, Kubernetes, Podman, etc.), providing resilient and scalable task execution.
- **Logging & Metrics**: Centralized production of logs and metrics for observability, auditing, and performance monitoring. Consumption/aggregation is externalized.
- **Extensibility**: Unified plugin framework for extending authentication, authorization, workflow registries, persistence, task executors, and event handlers.

## Key Features
- Ultra-fast and resilient workflow execution
- Secure, provider-based authentication and authorization
- Flexible task execution (shell, containers, scripts, etc.)
- Artifact management and data flow, including secrets
- Multi-tenancy via Workspaces and RBAC
- Comprehensive logging and metrics
- Extensible via provider/plugin architecture
- Native support for Work/Task testing

## Getting Started

Orkester targets Linux. The recommended workflow uses **Podman** (or Docker) to build and run on Linux from any host OS.

#### Build the dev image (once)
```bash
podman build --target dev -t orkester-dev -f docker/Dockerfile .
```

#### Day-to-day development
Start the dev container once. It mounts the project source and keeps a build cache volume so recompilation is incremental.

```bash
# Start the container (if not running) and open a shell — or re-attach if already running
./docker/dev.sh

# From inside the container — all standard Cargo commands work:
cargo check
cargo build
cargo test
cargo run -- --config-file config.yaml
```

You can also run a single command directly without entering the shell:
```bash
./docker/dev.sh cargo check
./docker/dev.sh cargo test
```

To stop the dev container when you're done for the day:
```bash
podman stop orkester-dev
```

#### Build & run the release image
```bash
podman build -t orkester -f docker/Dockerfile .
podman run --rm -p 8080:8080 -v ./plugins:/orkester/plugins:ro,z orkester
```

#### CLI reference
```
orkester [OPTIONS]

Options:
  -c, --config-file <PATH>   Path to a JSON, YAML or TOML configuration file
  -h, --help                 Print help
  -V, --version              Print version
```

#### Minimal config example (`config.yaml`)
```yaml
plugins:
  dir: ./plugins      # directory to scan for .so plugin files
  recursive: false    # scan sub-directories?
```

---

## Design Decisions & Scope
- **Eventing & Notifications**: Achievable using logs, specialized Tasks, and Artifacts; not a core feature but easily implemented.
- **UI/UX**: Not included in Orkester core; a separate Orkester-UI project will consume the REST API.
- **High Availability & Scalability**: Delegated to orchestration platforms (e.g., Kubernetes).
- **Audit Logging**: Orkester produces logs; aggregation and retention are external concerns.
- **Failure Handling**: Orkester is resilient; task failures are isolated and do not constitute system failure.
- **Secrets Management**: Handled as a special Artifact type, with support for secure injection and integration with external secret stores via plugins.

## Use Cases
- CI/CD pipelines
- Data processing workflows
- Automated operations and DevOps tasks
- Secure, auditable workflow automation

---

MIT OR Apache-2.0 License