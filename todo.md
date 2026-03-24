# TODO

## Host

- expose the component registry (using a Registry component ?)
    -> When loading a plugin, the host should list the available components in the Root component loaded to populate the registry
- Add logging with various levels (from TRACE to ERROR) (using the SDK with LogEntry requests once the hub is started!, before that, use a dedicated logger for the host)

## HUB

- use a map instead of a list of rules (the key is the name)
- produce better logging on config error, identifiying the problematic route/filter/target

## Rest

- Expose the full catalog (Namespaces, Groups, Works, Tasks, WorkerProfiles, TaskRunnerProfiles, Crons)
- Expose the full workflow (Workers, WorkRuns, TaskRuns)
- Use better URI (like /v1/catalog/.../, /v1/workflow/..., etc.)
- Add logging with various levels (from TRACE to ERROR) (using the SDK with LogEntry requests!)

## Workflows

- Add logging with various levels (from TRACE to ERROR) (using the SDK with LogEntry requests!)
- Dynamic pieces MUST be provided as component. This means there can be traits defining what a DocumentsLoader/DocumentParser/TaskRunner/... is, but then every implementation must be wrapped in a component (using the orkester SDK/macro). The goal is that they will be referenced as components (i.e. `kind: "workaholic/ShellRunner:1.0" for example) and created/used via the SDK (CreateComponent, etc.)
- Persistence is not a "called" system (I see you added handlers to persists things). It how the workflow server stores its data. So it should be automatic: the objects are stored and loaded automaticaly with the persistence provider.

## DEV

- Create a script in the dev folder that:
  1. Build both projects (orkester and workaholic)
  2. Copy the plugin SO (not the macro SO) files and the orkester binary in the /orkester/bin folder
  3. Run orkester with the config in the bin folder

- Ensure the config (workaholic.yaml and the resources in configs) is valid and has a testable set of resources

## Global

- Always ensure there is enough logs, with enough context, with the correct level, so that we can debug/investigate what is happening.
- Use the LocalFsPersistenceProvider when a persistence is required (workflow server) ! Must not be hardcoded, but set in the config.