IMPORTANT:

- To test: use podman `podman exec -w /orkester/run/workaholic orkester-dev cargo build` (you can adapt the workdir and the command)
- You can build an launch the orkester app with the workaholic.yaml config (in the bin folder) with `podman exec orkester-dev ./dev/build-and-run.sh`
- Ensure you handled all ToDos
- When writing code, use reusable, composable, unit elements (small objects and functions, less than 30lines of code per function, less than 200 lines per file, only imports/exports in mod.rs)

# TODO:

## Logging ✅

~~Implement the logging system in 3 parts~~

All 3 parts implemented and building.

Part 1: add minimalist logging to the SDK using the existing ABI interface.
- Expose LogLevel, LogRecord, init_logging, send_log, and macros log_trace/log_debug/log_info/log_warn/log_error.
- Macros must auto-fill file/module/line.
- The SDK logger must have 3 modes: Buffering, Connected, Fallback.
- Before the Logging Server is ready, logs go to a bounded startup ring buffer.
- On overflow, drop oldest and count dropped logs.
- When the Logging Server becomes available, flush buffered logs in order.
- On delivery failure, fallback to stderr/stdout.
- Components must never build envelopes manually to log.

Part 2: create crate orkester-plugin-logging implementing a full LoggingServer component.
- LoggingServer is a normal Orkester component.
- It receives structured log records from a dedicated host logging ingress, not from HUB envelopes.
- It supports dynamic/configurable sinks and formatters.
- Text sinks must use a formatter component.
- Provide standard sink components: ConsoleLogSink, LocalFsLogSink, S3LogSink.
- Provide standard formatter components: JsonLogFormatter, YamlLogFormatter, ConsoleLogFormatter.
- LocalFsLogSink and S3LogSink must support rotation.
- Implement bounded queues, per-source anti-spam policy, and metrics/counters.
- Keep sink and formatter modules isolated and small.

Part 3: adapt the host to bridge SDK logging, Logging Server and plugins.
- The host owns a global logging bridge.
- The Logging Server is loaded and started like any other plugin/component.
- The host stores a global reference/handle to the active Logging Server consumer.
- When plugins/components are loaded, initialize SDK logging automatically with plugin/component identity and ABI logging function/context.
- The host must buffer startup logs before the Logging Server is ready, then flush them when connected.
- Keep the logging path dedicated and separate from the HUB.

Do not move advanced logic into the SDK.
Do not make the Logging Server a special lifecycle component.
Preserve the existing plugin/component architecture.

## UI

- [x] UI -> Metrics (Snapshot + History Graphs)
    - [x] Display a nice Metrics Dashboard with Data 'Cards'
    - [x] Must not assume the metrics contain any specific key, it should by dynamic/flexible to handle the future keys
    - [x] Can use a library if needed (but keep it simple, no dependency management, just a single not-too-big JS lib)