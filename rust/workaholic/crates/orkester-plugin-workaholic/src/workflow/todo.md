TODO:

Loaders:
- [ ] Capture metrics like time used to sync documents (how many ms spent on each watch)

Workflow:
- [ ] Check the Cron feature (not working for now)
- [ ] Ensure the standard outputs (stdout, stderr) of the TaskRuns are captured and saved in the TaskRun state
- [ ] Ensure the inputs/ouputs used by a TaskRun are saved in the TaskRun state (the exact inputs/outpus as value or registry ref)

UI:
- [ ] Fix the Work modal to correctly show the graph, with details about the Work (spec/status/metadata) presented in a user-friendly way.
- [ ] Fix the WorkRun page to correctly show the steps stats. Add missing info about the WorkRun. Add missing info about TaskRuns (the ouputs, taskRun config, etc.)

Validation:
- [ ] Create a new testing Work with two Task running in local Container (with podman) (Simple python script looping a variable number of times passed by the first task) in the bin/configs (in a `test-container.yaml` file)

IMPORTANT:

- To test: use podman `podman exec -w /orkester/run/workaholic orkester-dev cargo build` (you can adapt the workdir and the command)
- You can build an launch the orkester app with the workaholic.yaml config (in the bin folder) with `podman exec orkester-dev ./dev/build-and-run.sh`