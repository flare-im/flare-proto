//! Tenant configuration that clients may see. A plugin marks top-level
//! properties of its `configurationSchema` with `x-flare-client-visible: true`
//! (for example the maximum number of poll options, or whether anonymous polls
//! are allowed). The platform sends only that subset with the client catalog
//! ([`crate::ui::ClientPlugin::configuration`]); everything else stays with the
//! plugin service. Visibility is not authorization: the plugin re-checks every
//! limit it publishes this way.
use crate::{ProtocolError, Result};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub const CLIENT_VISIBLE_KEYWORD: &str = "x-flare-client-visible";
/// Serialized size of the client-visible subset.
pub const MAX_CLIENT_CONFIGURATION_BYTES: usize = 4096;

/// Top-level property names marked client-visible.
pub fn client_visible_keys(schema: &Value) -> BTreeSet<String> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter(|(_, property)| {
                    property.get(CLIENT_VISIBLE_KEYWORD) == Some(&Value::Bool(true))
                })
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The client-visible subset of `configuration` (an object). Keys absent from
/// the configuration are absent from the subset.
pub fn client_visible_configuration(
    schema: &Value,
    configuration: &Value,
) -> Result<Map<String, Value>> {
    let configuration = configuration
        .as_object()
        .ok_or(ProtocolError::Denied("configuration must be an object"))?;
    let visible: Map<String, Value> = client_visible_keys(schema)
        .into_iter()
        .filter_map(|key| configuration.get(&key).map(|value| (key, value.clone())))
        .collect();
    if serde_json::to_vec(&visible).map_or(true, |v| v.len() > MAX_CLIENT_CONFIGURATION_BYTES) {
        return Err(ProtocolError::Denied("client configuration exceeds limit"));
    }
    Ok(visible)
}

/// The marker is a boolean on a top-level property and nowhere else, so a
/// nested marker never looks like it hides or exposes something it does not.
pub(crate) fn validate_client_visible_markers(schema: &Value) -> Result<()> {
    fn nested(value: &Value) -> bool {
        match value {
            Value::Object(object) => {
                object.contains_key(CLIENT_VISIBLE_KEYWORD) || object.values().any(nested)
            }
            Value::Array(items) => items.iter().any(nested),
            _ => false,
        }
    }
    let invalid = || ProtocolError::InvalidManifest {
        field: "configurationSchema".into(),
        reason: format!("{CLIENT_VISIBLE_KEYWORD} must be a boolean on a top-level property"),
    };
    let Value::Object(root) = schema else {
        return Ok(());
    };
    for (key, value) in root {
        if key == CLIENT_VISIBLE_KEYWORD {
            return Err(invalid());
        }
        if key != "properties" {
            if nested(value) {
                return Err(invalid());
            }
            continue;
        }
        let Some(properties) = value.as_object() else {
            continue;
        };
        for property in properties.values() {
            let Value::Object(property) = property else {
                continue;
            };
            for (name, child) in property {
                if name == CLIENT_VISIBLE_KEYWORD {
                    if !child.is_boolean() {
                        return Err(invalid());
                    }
                } else if nested(child) {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}
