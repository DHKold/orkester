/// Descriptive metadata attached to a component kind.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComponentMetadata {
    /// Fully-qualified kind identifier, e.g. `"example/EchoComponent:1.0"`.
    pub kind: String,
    /// Human-readable name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
}
