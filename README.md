# Orkester

Orkester is a Workflow Plateform with a pluggin system made in Rust.

## Features

TODO

## Quick Start

TODO

## Upcoming features

### Global

- [x] Produce a release rootless and distroless image
- [ ] Create documentation
- [x] Push to Github ?
- [ ] Refactor Plugins to ABI abstraction

### Commons

- [ ] Identity & Ownership
- [ ] Authentication support
- [ ] Authorization support
- [ ] LogFormatter support + JsonLogFormatter

### Core App

- Nothing for now

### Core Plugin

Workspace:
- [ ] Packages : Ability to group Tasks and Works in packages
- [ ] Historization : Ability to keep track of changes to Works and Tasks

Workflow:
- [x] ContainerTaskExecutor : Ability to run tasks in docker / podman
- [ ] OrkesterTaskExecutor : Ability to trigger Orkester commands (e.g. create workflows)
- [ ] Workflow archiving : Ability to archive workflows (remove logs, keep state and metrics)
- [ ] ThreadWorker : Ability to run a worker in a separate thread

Metrics:
- [ ] Work metrics : Add work specific metrics (like #Workflows, Total Time, etc.)
- [ ] Security metrics : Add metrics related to authentication & authorization

Security
- [ ] PasswordAuthenticationProvider
- [ ] JwtAuthenticationProvider
- [ ] FileAuthorizationProvider
- [ ] OPAAuthorizationProvider

Persistence
- [x] FilePersistenceProvider

### UI

- [ ] Identity & Security support

### Plugins

- [ ] OPA Plugin
  - [ ] OpaAuthorizationProvider
- [ ] SQL Plugin
  - [ ] SqlPersistenceProvider
  - [ ] SqlTaskExecutorProvider
  - [ ] SqlAuthenticationProvider
  - [ ] SqlAuthorizationProvider
  - [ ] SqlDocumentLoader
- [ ] AWS Plugin
  - [ ] S3PersistenceProvider
  - [ ] S3DocumentLoader
