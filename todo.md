IMPORTANT:

- To test: use podman `podman exec -w /orkester/run/workaholic orkester-dev cargo build` (you can adapt the workdir and the command)
- You can build an launch the orkester app with the workaholic.yaml config (in the bin folder) with `podman exec orkester-dev ./dev/build-and-run.sh`
- Ensure you handled all ToDos by ticking the boxes in this file once a todo is validated
- When writing code, use reusable, composable, unit elements (small objects and functions, less than 30lines of code per function, less than 200 lines per file, only imports/exports in mod.rs)

TODO:

- [ ] Logging Server
- [ ] UI -> Metrics (Snapshot + History Graphs)

LATER:

- [ ] Authentication Server
- [ ] Authorization Server
- [ ] Cleanup
- [ ] 