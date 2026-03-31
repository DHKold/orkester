use serde_json::Value;

/// Extract a named output value from job stdout.
///
/// Tries two conventions in order:
/// 1. A line of the form `<name>=<value>`.
/// 2. If `total_outputs == 1`, the last non-empty trimmed line.
pub fn extract_output_value(name: &str, stdout: &str, total_outputs: usize) -> Option<Value> {
    let prefix = format!("{name}=");
    for line in stdout.lines() {
        if let Some(val) = line.trim().strip_prefix(&prefix) {
            return Some(Value::String(val.to_string()));
        }
    }
    if total_outputs == 1 {
        stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .last()
            .map(|l| Value::String(l.trim().to_string()))
    } else {
        None
    }
}
