TODO:

Workflow:
- [ ] Fix the inputs/ouputs management of TaskRuns. Curenlty, it seems they are not well handled, as sho the log from the test-shell second task:

```log
stdout
Starting loop with count=work://steps/generate/outputs?count
Loop complete after work://steps/generate/outputs?count iterations.

stderr
seq: invalid floating point argument: 'work://steps/generate/outputs?count'
Try 'seq --help' for more information.
```

The inputs should have been resolved. This also means the ouputs must corectly handled to be accessible by other steps as in this example (the step2 expects the output 'count' from step1 as a work scoped StructuredData artifact)

UI:
- [ ] Fix the Cron page to correctly display the info of crons (it seems to be using the old model)
- [ ] Add a modal when clicking on a Task in the Catalog to show details of that task
- [ ] The Catalog should used a future-proof (there will be more kinds of resources in the future) kind of 'tabs' display instead of listing all types of resources on a single page.

Loaders:
- [ ] Implement an S3DocumentLoader

IMPORTANT:

- To test: use podman `podman exec -w /orkester/run/workaholic orkester-dev cargo build` (you can adapt the workdir and the command)
- You can build an launch the orkester app with the workaholic.yaml config (in the bin folder) with `podman exec orkester-dev ./dev/build-and-run.sh`
- Ensure you handled all ToDos by ticking the boxes in this file once a todo is validated
- When writing code, use reusable, composable, unit elements (small objects and functions, less than 30lines of code per function, less than 200 lines per file, only imports/exports in mod.rs)