use serde::{Deserialize, Serialize};

use crate::document::Document;

pub const GROUP_KIND: &str = "workaholic/Group:1.0";

pub type Group = Document<GroupSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupSpec;
