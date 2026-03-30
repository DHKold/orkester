TODO:

- [ ] Add a /v1/health endpoint
- [ ] Create a new crate `orkester-plugin-metrics` in the orkester Rust workspace.
- [ ] Create a new Metrics Server (production grade code and functionalities) in the new crate.
    - [ ] It consumes Metric Events (at least to SET, INCREASE, DECREASE, RESET data points)
    - [ ] It exposes metrics snapshot (so that it can be exposed by a REST Server to be consumed by the UI or by external tools like Prometheus etc.)
    - [ ] It can if enabled, build full history of metrics and expose it so that UI/Tools can produce Graphs etc about evolution of the metrics
    - [ ] Must be built using the orkester framework (Component) as it will be integrated with workaholic
- [ ] Update the Dockerfiles (Dockerfile and Dockerfile.dev) to package the new orkester + workaholic binaries (orkester executable + sample & metrics & workaholic plugins)
    - [ ] Must keep the rootless, distroless approach, it's just about packaging the correct binary and libs
    - [ ] Must embed the UI
    - [ ] The dev image should also embed the configs (in bin/configs) and the workaholic.yaml (in bin) files, can be relocated in better folders
- [ ] Review and update the helm-charts/orkester HELM Chart
    - [ ] Most probably, only the 'config' section must be fully adapted to the new config structure
    - [ ] It should have a default working configuration

IMPORTANT:

- To test: use podman `podman exec -w /orkester/run/workaholic orkester-dev cargo build` (you can adapt the workdir and the command)
- You can build an launch the orkester app with the workaholic.yaml config (in the bin folder) with `podman exec orkester-dev ./dev/build-and-run.sh`
- Ensure you handled all ToDos by ticking the boxes in this file once a todo is validated
- When writing code, use reusable, composable, unit elements (small objects and functions, less than 30lines of code per function, less than 200 lines per file, only imports/exports in mod.rs)