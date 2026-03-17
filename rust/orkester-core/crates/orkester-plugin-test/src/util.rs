use orkester_plugin::sdk::{Error, OwnedMessage, Result};

pub fn json_response<T: serde::Serialize>(id: u64, value: &T) -> Result<OwnedMessage> {
    let payload = serde_json::to_vec(value).map_err(|_| Error::Custom("json serialization failed"))?;
    Ok(OwnedMessage::new(
        id,
        orkester_plugin::abi::TYPE_JSON,
        orkester_plugin::abi::FLAG_RESPONSE,
        payload,
    ))
}

pub fn utf8_response(id: u64, text: impl Into<String>) -> OwnedMessage {
    OwnedMessage::new(
        id,
        orkester_plugin::abi::TYPE_UTF8,
        orkester_plugin::abi::FLAG_RESPONSE,
        text.into().into_bytes(),
    )
}