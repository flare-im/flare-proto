//! Data-only UI protocol shared by every client. No HTML, URLs, script, SQL,
//! expression evaluator, or dynamic native code is accepted.
use crate::{
    ProtocolError, Result,
    manifest::{ClientExtensions, ClientPlatform, UiContributionKind},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const UI_CATALOG_OPERATION: &str = "flare.plugin.ui.catalog";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiDocument {
    pub title: String,
    #[serde(default)]
    pub fields: Vec<UiField>,
    #[serde(default)]
    pub actions: Vec<UiAction>,
    #[serde(default)]
    pub translations: std::collections::BTreeMap<String, UiTranslation>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiTranslation {
    pub title: String,
    pub fields: Vec<String>,
    pub actions: std::collections::BTreeMap<String, String>,
    /// Display labels for enumerated field values, keyed by field path.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub values: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}
pub struct UiRenderContext<'a> {
    pub tenant: &'a str,
    pub platform: ClientPlatform,
    pub kind: UiContributionKind,
    pub locale: &'a str,
    /// The signed-in user the view is presented to; empty when unknown.
    pub viewer: &'a str,
    pub now_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiField {
    pub label: String,
    pub path: String,
    /// Display labels for enumerated values (a status shown as words, not as its wire value).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub values: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiAction {
    pub id: String,
    pub label: String,
    pub target: UiActionTarget,
    #[serde(default)]
    pub input: std::collections::BTreeMap<String, String>,
    /// Presentation only: the action is omitted when any condition matches. The
    /// plugin still authorizes every invocation; hiding is never a permission.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_when: Vec<UiCondition>,
}
/// A data-only test on the message payload. No expressions: a value list or the viewer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCondition {
    pub path: String,
    /// Matches when the value (text, number or bool) is one of these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub any_of: Vec<String>,
    /// Matches when the viewer is the value, or an element of an array value.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub viewer: bool,
    /// Inverts the match. A missing value never matches either way, so older
    /// payloads without the path keep the action.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub negate: bool,
}
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.chars().take(1024).collect()),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        _ => None,
    }
}
impl UiCondition {
    fn matches(&self, payload: &Value, viewer: &str) -> bool {
        let Some(value) = payload.pointer(&self.path).filter(|v| !v.is_null()) else {
            return false;
        };
        let hit = (!self.any_of.is_empty()
            && scalar_text(value).is_some_and(|text| self.any_of.contains(&text)))
            || (self.viewer
                && !viewer.is_empty()
                && match value {
                    Value::Array(items) => items.iter().any(|item| item.as_str() == Some(viewer)),
                    other => other.as_str() == Some(viewer),
                });
        hit != self.negate
    }
}
fn valid_path(path: &str) -> bool {
    path.len() <= 128
        && path.starts_with('/')
        && path
            .split('/')
            .skip(1)
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}
/// Exact locale first, then the same language (`en`, `en-GB` → `en-US`).
fn translation_for<'a>(
    translations: &'a std::collections::BTreeMap<String, UiTranslation>,
    locale: &str,
) -> Option<&'a UiTranslation> {
    if let Some(exact) = translations.get(locale) {
        return Some(exact);
    }
    let language = |tag: &str| {
        tag.split(['-', '_'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
    };
    let wanted = language(locale);
    (!wanted.is_empty())
        .then(|| {
            translations
                .iter()
                .find(|(tag, _)| language(tag) == wanted)
                .map(|(_, t)| t)
        })
        .flatten()
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum UiActionTarget {
    Operation { operation: String },
    Page { page: String },
    Native { capability: NativeCapability },
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum NativeCapability {
    SelectImage,
    SelectFile,
    Camera,
    Microphone,
    ClipboardWrite,
}
impl UiDocument {
    pub fn validate(&self) -> Result<()> {
        let valid_text =
            |s: &str| !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control);
        if !valid_text(&self.title) || self.fields.len() > 32 || self.actions.len() > 8 {
            return Err(ProtocolError::Denied("invalid UI document bounds"));
        }
        if self.translations.len() > 32
            || self.translations.iter().any(|(locale, t)| {
                locale.is_empty()
                    || locale.len() > 32
                    || !locale
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    || !valid_text(&t.title)
                    || t.fields.len() != self.fields.len()
                    || t.fields.iter().any(|label| !valid_text(label))
                    || t.actions.len() > self.actions.len()
                    || t.actions.iter().any(|(id, label)| {
                        !valid_text(label) || !self.actions.iter().any(|a| &a.id == id)
                    })
                    || t.values.len() > self.fields.len()
                    || t.values.iter().any(|(path, labels)| {
                        !self
                            .fields
                            .iter()
                            .any(|f| &f.path == path && !f.values.is_empty())
                            || labels.len() > 32
                            || labels.iter().any(|(value, label)| {
                                value.is_empty() || value.len() > 64 || !valid_text(label)
                            })
                    })
            })
        {
            return Err(ProtocolError::Denied("invalid UI translation"));
        }
        let mut ids = BTreeSet::new();
        for field in &self.fields {
            if !valid_text(&field.label)
                || !valid_path(&field.path)
                || field.values.len() > 32
                || field.values.iter().any(|(value, label)| {
                    value.is_empty() || value.len() > 64 || !valid_text(label)
                })
            {
                return Err(ProtocolError::Denied("invalid UI field"));
            }
        }
        for action in &self.actions {
            if action.input.len() > 32
                || action.input.iter().any(|(name, path)| {
                    name.is_empty()
                        || name.len() > 64
                        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || path.len() > 128
                        || !path.starts_with('/')
                        || path.split('/').skip(1).any(|s| {
                            s.is_empty()
                                || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        })
                })
            {
                return Err(ProtocolError::Denied("invalid UI action bindings"));
            }
            if action.hidden_when.len() > 4
                || action.hidden_when.iter().any(|c| {
                    !valid_path(&c.path)
                        || c.any_of.is_empty() != c.viewer
                        || c.any_of.len() > 16
                        || c.any_of.iter().any(|v| v.is_empty() || v.len() > 64)
                })
            {
                return Err(ProtocolError::Denied("invalid UI action condition"));
            }
            if !valid_text(&action.label)
                || action.id.is_empty()
                || action.id.len() > 64
                || !action
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !ids.insert(&action.id)
            {
                return Err(ProtocolError::Denied("invalid UI action"));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPlugin {
    pub plugin_id: String,
    pub version: String,
    pub manifest_digest: String,
    pub grant_revision: u64,
    pub extensions: ClientExtensions,
    pub operations: BTreeSet<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCatalog {
    pub tenant_id: String,
    pub version: u64,
    pub valid_until_ms: i64,
    pub plugins: Vec<ClientPlugin>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginMessage {
    pub plugin_id: String,
    pub contribution_id: String,
    pub schema_version: u32,
    pub summary: String,
    pub payload: Value,
}
impl PluginMessage {
    pub fn validate(&self) -> Result<()> {
        let identifier = |id: &str| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        };
        if !identifier(&self.plugin_id)
            || self.plugin_id.len() > 128
            || !identifier(&self.contribution_id)
            || self.contribution_id.len() > 160
            || !self
                .contribution_id
                .starts_with(&format!("{}.", self.plugin_id))
            || self.schema_version != 1
            || self.summary.len() > 1024
            || self.summary.chars().any(char::is_control)
            || !self.payload.is_object()
            || serde_json::to_vec(self).map_or(true, |v| v.len() > 65536)
        {
            return Err(ProtocolError::Denied("invalid plugin message"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedPluginView {
    pub available: bool,
    pub title: String,
    pub fields: Vec<ResolvedUiField>,
    pub actions: Vec<UiAction>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedUiField {
    pub label: String,
    pub value: String,
}
impl UiCatalog {
    /// Summary is inert text from message history. Never derive actions from it
    /// or from the historical payload, even when a newer plugin is installed.
    pub fn resolve_message(
        &self,
        tenant: &str,
        platform: ClientPlatform,
        message: &PluginMessage,
        now_ms: i64,
    ) -> ResolvedPluginView {
        self.resolve_with_context(
            message,
            UiRenderContext {
                tenant,
                platform,
                kind: UiContributionKind::MessageContent,
                locale: "",
                viewer: "",
                now_ms,
            },
        )
    }
    pub fn resolve_with_context(
        &self,
        message: &PluginMessage,
        context: UiRenderContext<'_>,
    ) -> ResolvedPluginView {
        let UiRenderContext {
            tenant,
            platform,
            kind,
            locale,
            viewer,
            now_ms,
        } = context;
        let fallback = || ResolvedPluginView {
            available: false,
            title: message
                .summary
                .chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect(),
            fields: vec![],
            actions: vec![],
        };
        if self.tenant_id != tenant || self.valid_until_ms <= now_ms || message.validate().is_err()
        {
            return fallback();
        }
        let Some(plugin) = self.plugins.iter().find(|p| {
            p.plugin_id == message.plugin_id && p.extensions.platforms.contains(&platform)
        }) else {
            return fallback();
        };
        let Some(contribution) = plugin.extensions.contributions.iter().find(|c| {
            c.id == message.contribution_id
                && c.schema_version == message.schema_version
                && c.kind == kind
        }) else {
            return fallback();
        };
        if contribution.schema.validate().is_err() {
            return fallback();
        }
        let schema = &contribution.schema;
        let translation = translation_for(&schema.translations, locale);
        // A field without a value is not shown: a card says what it knows, not an empty row.
        let fields = schema
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                let raw = message.payload.pointer(&field.path).and_then(scalar_text)?;
                if raw.trim().is_empty() {
                    return None;
                }
                let translated_value = translation
                    .and_then(|t| t.values.get(&field.path))
                    .and_then(|labels| labels.get(&raw));
                let value = translated_value
                    .or_else(|| field.values.get(&raw))
                    .cloned()
                    .unwrap_or(raw);
                let label = translation
                    .and_then(|t| t.fields.get(index))
                    .unwrap_or(&field.label)
                    .clone();
                Some(ResolvedUiField { label, value })
            })
            .collect();
        let actions = schema
            .actions
            .iter()
            .filter(|action| {
                !action
                    .hidden_when
                    .iter()
                    .any(|condition| condition.matches(&message.payload, viewer))
            })
            .map(|action| {
                let mut action = action.clone();
                action.hidden_when.clear();
                if let Some(label) = translation.and_then(|t| t.actions.get(&action.id)) {
                    action.label = label.clone();
                }
                action
            })
            .collect();
        ResolvedPluginView {
            available: true,
            title: translation.map_or(&schema.title, |t| &t.title).clone(),
            fields,
            actions,
        }
    }
    /// This validates declaration and host consent, not server-side authorization.
    /// Invoke again through Core; it checks current grants, flags and actor policy.
    #[allow(clippy::too_many_arguments)]
    pub fn action(
        &self,
        tenant: &str,
        plugin_id: &str,
        contribution_id: &str,
        action_id: &str,
        platform: ClientPlatform,
        native_allowlist: &BTreeSet<NativeCapability>,
        user_gesture: bool,
        now_ms: i64,
    ) -> Result<UiActionTarget> {
        if tenant != self.tenant_id || now_ms >= self.valid_until_ms || !user_gesture {
            return Err(ProtocolError::Denied("UI action unavailable"));
        }
        let p = self
            .plugins
            .iter()
            .find(|p| p.plugin_id == plugin_id && p.extensions.platforms.contains(&platform))
            .ok_or(ProtocolError::Denied("UI plugin unavailable"))?;
        let c = p
            .extensions
            .contributions
            .iter()
            .find(|c| c.id == contribution_id && c.schema_version == 1)
            .ok_or(ProtocolError::Denied("UI contribution unavailable"))?;
        c.schema.validate()?;
        let a = c
            .schema
            .actions
            .iter()
            .find(|a| a.id == action_id)
            .ok_or(ProtocolError::Denied("UI action unavailable"))?;
        match &a.target {
            UiActionTarget::Operation { operation } if !p.operations.contains(operation) => {
                return Err(ProtocolError::Denied("UI operation unavailable"));
            }
            UiActionTarget::Page { page }
                if !p
                    .extensions
                    .contributions
                    .iter()
                    .any(|c| &c.id == page && c.kind == UiContributionKind::Page) =>
            {
                return Err(ProtocolError::Denied("UI page unavailable"));
            }
            UiActionTarget::Native { capability }
                if !native_allowlist.contains(capability)
                    || !p.extensions.native_capabilities.contains(capability) =>
            {
                return Err(ProtocolError::Denied("native capability unavailable"));
            }
            _ => {}
        }
        Ok(a.target.clone())
    }
}

/// Copy only manifest-declared input fields. Historical data cannot inject
/// tenant/actor/permissions or extra parameters into a capability command.
pub fn bind_action_input(action: &UiAction, payload: &Value) -> Result<Value> {
    let mut output = serde_json::Map::new();
    for (name, path) in &action.input {
        let value = payload
            .pointer(path)
            .ok_or(ProtocolError::Denied("UI input unavailable"))?;
        output.insert(name.clone(), value.clone());
    }
    let output = Value::Object(output);
    if serde_json::to_vec(&output).map_or(true, |v| v.len() > 16384) {
        return Err(ProtocolError::Denied("UI input limit"));
    }
    Ok(output)
}
