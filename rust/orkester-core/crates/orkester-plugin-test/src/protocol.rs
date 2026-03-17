use serde::{Deserialize, Serialize};

pub const TYPE_UTF8: u32 = orkester_plugin::abi::TYPE_UTF8;
pub const TYPE_JSON: u32 = orkester_plugin::abi::TYPE_JSON;

pub const MSG_GET_PLUGIN_METADATA: u32 = 1_000;
pub const MSG_LIST_COMPONENTS: u32 = 1_001;
pub const MSG_CREATE_COMPONENT: u32 = 1_002;

pub const MSG_COMPONENT_INVOKE: u32 = 2_000;
pub const MSG_COMPONENT_DELETE: u32 = 2_001;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateComponentRequest {
    pub component_id: String,
    pub config: serde_json::Value,
}