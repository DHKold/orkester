use serde::Deserialize;

use workaholic::{EntityKey, EntityValue};

#[derive(Deserialize)]
pub struct PersistorPutRequest {
    pub key: EntityKey,
    pub value: EntityValue,
}