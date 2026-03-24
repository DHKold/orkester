use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type Group = Document<GroupSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
