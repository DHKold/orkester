use serde::Deserialize;

use workaholic::{EntityKey, EntityValue};

#[derive(serde::Deserialize)]
pub struct PersistorPutRequest {
    pub key: EntityKey,
    pub value: EntityValue,
}