TODO:

TaskRunners:
- [x] Use common Documents
- [x] Implement the ShellTaskRunner Component
- [x] Implement HttpTaskRunner          + Component
- [x] Implement ContainerTaskRunner     + Component
- [x] Implement KubernetesTaskRunner    + Component

WorkRunners:
- [x] Use common Documents
- [x] Implement the ThreadWorkRunner Component

Workflow:
- [x] Implement the Workflow Server     + Component
- [x] Implement the Cron feature
- [x] Implement the Manual Trigger feature
- [x] Implement the Trigger Resolving feature
- [x] Ensure the Workflow Server follows the plan (plan.md)
- [x] Ensure the Workflow Server is correctly using the orkester framework, the workaholic library, the catalog interface, etc.

UI:
- [x] Adapt the UI to be able to use the Workflow Server

Validation:
- [x] Create a new testing Work with two Task running in Kubernetes (Simple python script looping a variable number of times passed by the first task) in the bin/configs (in a `test-kube.yaml` file)
- [x] Create a new testing Work with two Task running in Kubernetes (Simple bash script looping a variable number of times passed by the first task) in the bin/configs (in a `test-shell.yaml` file)
