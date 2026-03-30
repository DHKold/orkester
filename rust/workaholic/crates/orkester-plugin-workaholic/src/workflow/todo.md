TODO:

WorkRuns:
- [ ] Logs related to the WorkRun should be put in the WorkRun state, allowing to trace what exactly happened when running the WorkRun (including failures, warnings, traces, etc.)

Runners:
- [ ] Check why the task `container-validate-count` is failing. This can be investigated more easily once WorkRuns hold lods (and more logs are produced by the WorRunner / TaskRunner)

UI:
- [ ] Find a better way to display the steps in the WorkRun page (there could be tens of steps, having a long list is not user friendly).
- [ ] Add a way to reset the Graph display (when manipulating it, it can become hard to handle)

IMPORTANT:

- To test: use podman `podman exec -w /orkester/run/workaholic orkester-dev cargo build` (you can adapt the workdir and the command)
- You can build an launch the orkester app with the workaholic.yaml config (in the bin folder) with `podman exec orkester-dev ./dev/build-and-run.sh`
- Ensure you handled all ToDos by ticking the boxes in this file once a todo is validated
- When writing code, use reusable, composable, unit elements (small objects and functions, less than 30lines of code per function, less than 200 lines per file, only imports/exports in mod.rs)