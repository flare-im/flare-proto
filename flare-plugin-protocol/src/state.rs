//! Shared state of a plugin entity (one poll, one solitaire): the presentation
//! every member of the conversation sees, on every device, after reconnects.
//!
//! A plugin publishes it in two ways, and Core validates both the same way
//! before it becomes an `EVENT_MESSAGE_PLUGIN_STATE` event:
//! - in the result of a user call, as `data.sharedState` (see
//!   [`SharedState::from_result`]);
//! - on its own, through the host write capability
//!   `im.plugin.entity_state.publish` (see
//!   [`crate::resource::ResourceWriteRequest`]), for changes no user call
//!   carries: a deadline passing, staff closing the entity from the console.
//!
//! `values` is a complete snapshot, not a patch. Clients overlay it on the card
//! payload (`payload ← shared ← local`, see [`crate::ui::UiStateLayers`]); like
//! every other UI input it grants nothing.
use crate::{ProtocolError, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Keys in [`SharedState::values`].
pub const MAX_SHARED_STATE_KEYS: usize = 32;
/// Serialized size of [`SharedState::values`].
pub const MAX_SHARED_STATE_BYTES: usize = 8 * 1024;
/// A string value.
pub const MAX_SHARED_STATE_STRING_BYTES: usize = 1024;
/// Items of an array value.
pub const MAX_SHARED_STATE_ARRAY_ITEMS: usize = 100;
/// A string item of an array value.
pub const MAX_SHARED_STATE_ITEM_BYTES: usize = 256;
/// An entity id.
pub const MAX_ENTITY_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedState {
    /// The entity id, equal to the card payload value at the message-content
    /// contribution's `entityPath`. Core accepts it only when it is bound to a
    /// message of the same plugin in the same tenant.
    pub entity: String,
    /// Strictly increasing per `(tenant, plugin, entity)`, `1..=i64::MAX`. Core
    /// drops an older version and treats the same version as a duplicate.
    pub version: u64,
    /// Keys `[A-Za-z0-9_]{1,64}`. Values are scalars (null, bool, number,
    /// string), arrays of strings or arrays of numbers. Null removes the payload
    /// key on overlay.
    pub values: Map<String, Value>,
}

/// How a received version compares with the stored one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedStateOrder {
    /// Apply it and announce the change.
    Newer,
    /// Already applied: idempotent, no redraw.
    Same,
    /// Out of order: drop it.
    Older,
}

/// Entity ids: `[A-Za-z0-9._:-]{1,128}`.
pub fn valid_entity_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ENTITY_ID_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}

pub(crate) fn valid_state_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl SharedState {
    pub fn validate(&self) -> Result<()> {
        let text = |s: &str, max: usize| s.len() <= max && !s.chars().any(char::is_control);
        let valid_value = |value: &Value| match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => true,
            Value::String(s) => text(s, MAX_SHARED_STATE_STRING_BYTES),
            Value::Array(items) => {
                items.len() <= MAX_SHARED_STATE_ARRAY_ITEMS
                    && (items.iter().all(|i| {
                        i.as_str()
                            .is_some_and(|s| text(s, MAX_SHARED_STATE_ITEM_BYTES))
                    }) || items.iter().all(Value::is_number))
            }
            Value::Object(_) => false,
        };
        if !valid_entity_id(&self.entity)
            || self.version == 0
            || self.version > i64::MAX as u64
            || self.values.len() > MAX_SHARED_STATE_KEYS
            || self
                .values
                .iter()
                .any(|(key, value)| !valid_state_key(key) || !valid_value(value))
            || serde_json::to_vec(&self.values).map_or(true, |v| v.len() > MAX_SHARED_STATE_BYTES)
        {
            return Err(ProtocolError::Denied("invalid plugin shared state"));
        }
        Ok(())
    }

    /// The state a successful call result asks Core to publish:
    /// `data.sharedState`. `Ok(None)` when absent or null; an error when present
    /// but malformed, so the host can log it instead of silently dropping it.
    ///
    /// Core publishes it only for an entity already bound to a stored card, so
    /// the result that creates the card (`data.message`) carries its initial
    /// state in the card payload instead. The caller's own client applies the
    /// returned state at once as its shared layer; the broadcast of the same
    /// version is then a duplicate.
    pub fn from_result(data: &Value) -> Result<Option<Self>> {
        let Some(raw) = data.get("sharedState").filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        let state: Self = serde_json::from_value(raw.clone())
            .map_err(|_| ProtocolError::Denied("invalid plugin shared state"))?;
        state.validate()?;
        Ok(Some(state))
    }

    /// Rebuild the state from the wire parts of `PluginStateEvent` or a stored
    /// row. The JSON is bounded before it is parsed.
    pub fn from_parts(entity: &str, version: u64, values_json: &str) -> Result<Self> {
        if values_json.len() > MAX_SHARED_STATE_BYTES {
            return Err(ProtocolError::Denied("invalid plugin shared state"));
        }
        let values: Map<String, Value> = serde_json::from_str(values_json)
            .map_err(|_| ProtocolError::Denied("invalid plugin shared state"))?;
        let state = Self {
            entity: entity.to_owned(),
            version,
            values,
        };
        state.validate()?;
        Ok(state)
    }

    /// `values` as the `values_json` of `PluginStateEvent`.
    pub fn values_json(&self) -> String {
        Value::Object(self.values.clone()).to_string()
    }

    pub fn order_against(&self, stored_version: Option<u64>) -> SharedStateOrder {
        match stored_version {
            Some(stored) if self.version < stored => SharedStateOrder::Older,
            Some(stored) if self.version == stored => SharedStateOrder::Same,
            _ => SharedStateOrder::Newer,
        }
    }

    /// Shallow overlay: a value replaces the top-level payload key, null removes it.
    pub fn merge_into(&self, payload: &Value) -> Value {
        overlay(payload, &self.values)
    }
}

pub(crate) fn overlay(payload: &Value, values: &Map<String, Value>) -> Value {
    let mut merged = payload.as_object().cloned().unwrap_or_default();
    for (key, value) in values {
        if value.is_null() {
            merged.remove(key);
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}
