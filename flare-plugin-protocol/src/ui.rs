//! Data-only UI protocol shared by every client. No HTML, URLs, script, SQL,
//! expression evaluator, or dynamic native code is accepted.
//!
//! Version 2 of the document is additive and keeps `schemaVersion: 1`: display
//! `blocks`, form `inputs`, action `form`/`confirm`/`style`, `exists`
//! conditions, `texts` translations, the contribution `detail`, and resolution
//! over two state layers ([`UiStateLayers`]). Blocks and the detail take
//! `hiddenWhen` conditions like actions. Later additions are optional in the
//! same way: `allOf` conditions, static choices `options`, entries
//! `userIdsPath`, rows `filters`, input `maxPath` and detail paging
//! (`cursorPath`). A composer-action document resolves against
//! [`composer_data`]. Older documents parse, validate and resolve exactly as
//! before.
mod blocks;
mod inputs;
pub use blocks::*;
pub use inputs::*;

use crate::{
    ProtocolError, Result,
    manifest::{ClientExtensions, ClientPlatform, UiContribution, UiContributionKind},
    state::SharedState,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const UI_CATALOG_OPERATION: &str = "flare.plugin.ui.catalog";
/// Document bounds.
pub const MAX_UI_FIELDS: usize = 32;
pub const MAX_UI_ACTIONS: usize = 16;
pub const MAX_UI_BLOCKS: usize = 24;
pub const MAX_UI_INPUTS: usize = 16;
/// Labels, titles, placeholders and confirmations: bytes, no control characters.
pub const MAX_UI_TEXT_BYTES: usize = 256;
/// Literal text of text and note blocks: bytes, newlines allowed.
pub const MAX_UI_LONG_TEXT_BYTES: usize = 2000;
/// Entries of one translation's `texts`.
pub const MAX_UI_TRANSLATED_TEXTS: usize = 256;
/// Serialized size of an action input (declared input plus collected values).
pub const MAX_UI_ACTION_INPUT_BYTES: usize = 16384;
/// Key of the client-visible configuration in the data of a composer-action
/// document (see [`composer_data`]).
pub const COMPOSER_CONFIG_KEY: &str = "config";
/// Input name of the page cursor sent to a paged detail operation.
pub const DETAIL_CURSOR_INPUT: &str = "cursor";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiDocument {
    pub title: String,
    #[serde(default)]
    pub fields: Vec<UiField>,
    #[serde(default)]
    pub actions: Vec<UiAction>,
    /// Display blocks, in order (`0..=24`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<UiBlock>,
    /// Form inputs referenced by action forms, and admin-page filters (the
    /// rows block's `filters`, or every input when it lists none) (`0..=16`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<UiInput>,
    #[serde(default)]
    pub translations: BTreeMap<String, UiTranslation>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiTranslation {
    pub title: String,
    pub fields: Vec<String>,
    pub actions: BTreeMap<String, String>,
    /// Display labels for enumerated field values, keyed by field path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, BTreeMap<String, String>>,
    /// Every other text of the document, keyed by its source text: block
    /// labels and literals, value labels of blocks and columns, input labels,
    /// placeholders and choices, action confirmations. A text without an entry
    /// is shown as declared.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub texts: BTreeMap<String, String>,
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
/// The two state layers a card is resolved with, over its payload:
/// `payload ← shared ← local`. Each is a shallow overlay of top-level keys in
/// which null removes the key.
///
/// - `shared`: the entity's [`SharedState`] from Core (events and sync). It
///   applies only when the payload names the same entity at the
///   contribution's `entityPath`.
/// - `local`: the [`PluginMessageState`] from the viewer's own call results,
///   kept on this device only (such as `mySelection`). A local key replaces a
///   shared key of the same name, so plugins keep viewer-specific keys apart
///   from shared ones.
///
/// An invalid layer is ignored.
#[derive(Debug, Clone, Copy, Default)]
pub struct UiStateLayers<'a> {
    pub shared: Option<&'a SharedState>,
    pub local: Option<&'a PluginMessageState>,
}
impl UiStateLayers<'_> {
    /// The data a card of `contribution` is resolved against.
    pub fn merge(&self, contribution: &UiContribution, payload: &Value) -> Value {
        let mut data = payload.clone();
        if let Some(shared) = self.shared.filter(|shared| {
            shared.validate().is_ok()
                && contribution.entity_of(payload).as_deref() == Some(shared.entity.as_str())
        }) {
            data = shared.merge_into(&data);
        }
        if let Some(local) = self.local.filter(|local| local.validate().is_ok()) {
            data = local.merge_into(&data);
        }
        data
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiField {
    pub label: String,
    pub path: String,
    /// Display labels for enumerated values (a status shown as words, not as its wire value).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UiActionStyle {
    Primary,
    Danger,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiAction {
    pub id: String,
    pub label: String,
    pub target: UiActionTarget,
    #[serde(default)]
    pub input: BTreeMap<String, String>,
    /// Presentation only: the action is omitted when any condition matches. The
    /// plugin still authorizes every invocation; hiding is never a permission.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_when: Vec<UiCondition>,
    /// Ids of document inputs the host collects first (`0..=16`, operation
    /// targets only); their validated values join the action input under their
    /// ids, which must not repeat a declared input name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form: Vec<String>,
    /// Asked before invoking (translated through `texts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<UiActionStyle>,
}
/// A data-only test on the data the document is resolved against (for a row
/// action, on the row). No expressions: a value list, the viewer, presence, or
/// all of a few such tests. Exactly one of `anyOf`, `viewer`, `exists` and
/// `allOf` is given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCondition {
    /// The value `anyOf`, `viewer` and `exists` test; required with them and
    /// absent with `allOf`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Matches when the value (text, number or bool) is one of these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub any_of: Vec<String>,
    /// Matches when the viewer is the value, or an element of an array value.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub viewer: bool,
    /// Matches when the value is present and not empty (null, "", [] and {}
    /// are empty; false and 0 are present).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exists: bool,
    /// Matches when every one of these conditions matches (`1..=4`, each with
    /// its own `path`). One level only: they may not hold `allOf` themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_of: Vec<UiCondition>,
    /// Inverts the match. For `anyOf` and `viewer` a missing value never
    /// matches either way, so older payloads without the path keep the action;
    /// a negated `exists` matches a missing value; a negated `allOf` matches
    /// when not every condition matches (each still follows its own rules).
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
    pub(crate) fn is_valid(&self) -> bool {
        self.is_valid_at(true)
    }
    fn is_valid_at(&self, top_level: bool) -> bool {
        let one_test = [
            !self.any_of.is_empty(),
            self.viewer,
            self.exists,
            !self.all_of.is_empty(),
        ]
        .into_iter()
        .filter(|test| *test)
        .count()
            == 1;
        if !one_test {
            return false;
        }
        if !self.all_of.is_empty() {
            return top_level
                && self.path.is_none()
                && self.all_of.len() <= MAX_UI_CONDITIONS
                && self.all_of.iter().all(|c| c.is_valid_at(false));
        }
        self.path.as_deref().is_some_and(valid_path)
            && self.any_of.len() <= 16
            && self.any_of.iter().all(|v| !v.is_empty() && v.len() <= 64)
    }
    pub(crate) fn matches(&self, data: &Value, viewer: &str) -> bool {
        if !self.all_of.is_empty() {
            return self.all_of.iter().all(|c| c.matches(data, viewer)) != self.negate;
        }
        let value = self
            .path
            .as_deref()
            .and_then(|path| data.pointer(path))
            .filter(|v| !v.is_null());
        if self.exists {
            let present = value.is_some_and(|value| match value {
                Value::String(s) => !s.is_empty(),
                Value::Array(items) => !items.is_empty(),
                Value::Object(object) => !object.is_empty(),
                _ => true,
            });
            return present != self.negate;
        }
        let Some(value) = value else {
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
/// JSON pointer of at most 128 bytes with non-empty `[A-Za-z0-9_]` segments.
pub(crate) fn valid_path(path: &str) -> bool {
    path.len() <= 128
        && path.starts_with('/')
        && path
            .split('/')
            .skip(1)
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}
pub(crate) fn valid_text(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= MAX_UI_TEXT_BYTES && !s.chars().any(char::is_control)
}
pub(crate) fn valid_long_text(s: &str) -> bool {
    !s.trim().is_empty()
        && s.len() <= MAX_UI_LONG_TEXT_BYTES
        && !s.chars().any(|c| c.is_control() && c != '\n')
}
/// Action input names and input ids: `[A-Za-z0-9_]{1,64}`.
pub(crate) fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
pub(crate) fn valid_field(field: &UiField) -> bool {
    valid_text(&field.label)
        && valid_path(&field.path)
        && field.values.len() <= 32
        && field
            .values
            .iter()
            .all(|(value, label)| !value.is_empty() && value.len() <= 64 && valid_text(label))
}
fn valid_bindings(input: &BTreeMap<String, String>) -> bool {
    input.len() <= 32
        && input
            .iter()
            .all(|(name, path)| valid_name(name) && valid_path(path))
}
/// Exact locale first, then the same language (`en`, `en-GB` → `en-US`).
fn translation_for<'a>(
    translations: &'a BTreeMap<String, UiTranslation>,
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
/// `source` in the view language, through the translation's `texts`.
pub(crate) fn translate(translation: Option<&UiTranslation>, source: &str) -> String {
    translation
        .and_then(|t| t.texts.get(source))
        .map_or(source, String::as_str)
        .to_owned()
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum UiConversationType {
    Single,
    Group,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum UiGroupRole {
    Owner,
    Admin,
    Member,
}
/// Who sees a composer-action entry. An empty list admits everyone; roles apply
/// in group conversations only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiAudience {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversation_types: Vec<UiConversationType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_roles: Vec<UiGroupRole>,
}
impl UiAudience {
    pub fn validate(&self) -> Result<()> {
        let types: BTreeSet<_> = self.conversation_types.iter().collect();
        let roles: BTreeSet<_> = self.group_roles.iter().collect();
        if types.len() != self.conversation_types.len()
            || roles.len() != self.group_roles.len()
            || (!self.group_roles.is_empty()
                && self.conversation_types == [UiConversationType::Single])
        {
            return Err(ProtocolError::Denied("invalid UI audience"));
        }
        Ok(())
    }
    /// Whether the entry is shown in a conversation of `conversation` type to a
    /// viewer with `role` there (`None` when the host does not know it; a group
    /// entry restricted by role is then hidden).
    pub fn admits(&self, conversation: UiConversationType, role: Option<UiGroupRole>) -> bool {
        (self.conversation_types.is_empty() || self.conversation_types.contains(&conversation))
            && (conversation != UiConversationType::Group
                || self.group_roles.is_empty()
                || role.is_some_and(|role| self.group_roles.contains(&role)))
    }
}

/// The "view all" document of a message-content or admin-page contribution.
/// The host invokes `operation` with `input` bound from the card data (or, on
/// an admin page, from the row), then resolves `document` against the result
/// `data` and shows it in a sheet. A detail document has no rows block and no
/// detail of its own.
///
/// Paging (`cursorPath`): the first page is requested with the bound input
/// alone; the resolved view carries the cursor of the next page as
/// `nextCursor`, and the host requests that page with the same input plus
/// `cursor` ([`Self::page_input`]), resolves it against the same document and
/// appends it ([`ResolvedPluginView::append_page`]). No `nextCursor` means
/// the last page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiDetail {
    /// Wire operation declared by the plugin.
    pub operation: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub input: BTreeMap<String, String>,
    /// The detail is not offered while any condition matches (`0..=4`), read
    /// on the data the contribution's document is resolved against: the
    /// merged card data, or an admin page's source result (not a row). The
    /// resolved view then has `detail: false`, entries blocks report no
    /// `more` (as without a detail), and rows carry no `detailInput`.
    /// Presentation only: the detail operation still authorizes every call.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_when: Vec<UiCondition>,
    /// The cursor of the next page in the detail result: a non-blank scalar of
    /// at most 256 bytes without control characters; anything else, or no
    /// value, ends paging. When set, `input` may not bind `cursor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_path: Option<String>,
    pub document: UiDocument,
}
/// A page cursor a host may send back: non-blank, at most 256 bytes, no
/// control characters.
fn valid_page_cursor(cursor: &str) -> bool {
    !cursor.trim().is_empty()
        && cursor.len() <= MAX_ROWS_CURSOR_BYTES
        && !cursor.chars().any(char::is_control)
}
impl UiDetail {
    pub fn validate(&self) -> Result<()> {
        if self.operation.is_empty()
            || self.operation.len() > 128
            || !valid_bindings(&self.input)
            || !blocks::valid_conditions(&self.hidden_when)
            || self.document.rows().is_some()
            || self.cursor_path.as_deref().is_some_and(|path| {
                !valid_path(path) || self.input.contains_key(DETAIL_CURSOR_INPUT)
            })
        {
            return Err(ProtocolError::Denied("invalid UI detail"));
        }
        self.document.validate()
    }
    /// The detail operation input, bound from `data`.
    pub fn bind_input(&self, data: &Value) -> Result<Value> {
        bind_inputs(&self.input, data)
    }
    /// The input of the page after `cursor` (a resolved view's `nextCursor`):
    /// `bound`, the first page's input ([`Self::bind_input`], or an admin
    /// row's `detailInput`), plus `cursor`. Only for a detail with
    /// `cursorPath`; the first page is requested without `cursor`.
    pub fn page_input(&self, bound: Value, cursor: &str) -> Result<Value> {
        let Value::Object(mut input) = bound else {
            return Err(ProtocolError::Denied("invalid UI detail page"));
        };
        if self.cursor_path.is_none()
            || !valid_page_cursor(cursor)
            || input.contains_key(DETAIL_CURSOR_INPUT)
        {
            return Err(ProtocolError::Denied("invalid UI detail page"));
        }
        input.insert(DETAIL_CURSOR_INPUT.into(), Value::from(cursor));
        bounded_input(input)
    }
    /// The next-page cursor in a detail result `data`, when it is one.
    fn next_cursor(&self, data: &Value) -> Option<String> {
        data.pointer(self.cursor_path.as_deref()?)
            .and_then(scalar_text)
            .filter(|cursor| valid_page_cursor(cursor))
    }
    /// Whether `hiddenWhen` hides the detail on a document resolved against
    /// `data` for `viewer`.
    pub fn is_hidden(&self, data: &Value, viewer: &str) -> bool {
        self.hidden_when.iter().any(|c| c.matches(data, viewer))
    }
}

impl UiDocument {
    /// The title in `locale`, with the same language fallback as resolved views
    /// (exact tag, then same language); the declared title otherwise.
    pub fn localized_title(&self, locale: &str) -> &str {
        translation_for(&self.translations, locale).map_or(&self.title, |t| &t.title)
    }
    pub fn validate(&self) -> Result<()> {
        if !valid_text(&self.title)
            || self.fields.len() > MAX_UI_FIELDS
            || self.actions.len() > MAX_UI_ACTIONS
            || self.blocks.len() > MAX_UI_BLOCKS
            || self.inputs.len() > MAX_UI_INPUTS
        {
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
                    || t.texts.len() > MAX_UI_TRANSLATED_TEXTS
                    || t.texts
                        .iter()
                        .any(|(source, text)| !valid_long_text(source) || !valid_long_text(text))
            })
        {
            return Err(ProtocolError::Denied("invalid UI translation"));
        }
        if !self.fields.iter().all(valid_field) {
            return Err(ProtocolError::Denied("invalid UI field"));
        }
        let mut input_ids = BTreeSet::new();
        for input in &self.inputs {
            input.validate()?;
            if !input_ids.insert(input.id.as_str()) {
                return Err(ProtocolError::Denied("invalid UI input"));
            }
        }
        let mut ids = BTreeSet::new();
        for action in &self.actions {
            if !valid_bindings(&action.input) {
                return Err(ProtocolError::Denied("invalid UI action bindings"));
            }
            if action.hidden_when.len() > 4 || !action.hidden_when.iter().all(UiCondition::is_valid)
            {
                return Err(ProtocolError::Denied("invalid UI action condition"));
            }
            let mut form = BTreeSet::new();
            if action.form.len() > MAX_UI_INPUTS
                || action.form.iter().any(|id| {
                    !input_ids.contains(id.as_str())
                        || !form.insert(id)
                        || action.input.contains_key(id)
                })
                || (!action.form.is_empty()
                    && !matches!(action.target, UiActionTarget::Operation { .. }))
            {
                return Err(ProtocolError::Denied("invalid UI action form"));
            }
            if !valid_text(&action.label)
                || action.confirm.as_deref().is_some_and(|c| !valid_text(c))
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
        let mut owned = BTreeSet::new();
        for block in &self.blocks {
            block.validate(&self.actions, &self.inputs)?;
            if !block.owned_actions().into_iter().all(|id| owned.insert(id)) {
                return Err(ProtocolError::Denied("invalid UI block"));
            }
        }
        Ok(())
    }
    /// The rows block, if any (admin-page documents hold at most one).
    pub fn rows(&self) -> Option<&UiRowsBlock> {
        self.blocks.iter().find_map(|block| match block {
            UiBlock::Rows(rows) => Some(rows),
            _ => None,
        })
    }
    pub(crate) fn rows_sources(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().filter_map(|block| match block {
            UiBlock::Rows(rows) => Some(rows.source.as_str()),
            _ => None,
        })
    }
    /// The choices block that owns `action_id`, for its selection input.
    pub fn choices_for(&self, action_id: &str) -> Option<&UiChoicesBlock> {
        self.blocks.iter().find_map(|block| match block {
            UiBlock::Choices(choices) if choices.action.as_deref() == Some(action_id) => {
                Some(choices)
            }
            _ => None,
        })
    }
    fn owned_actions(&self) -> BTreeSet<&str> {
        self.blocks
            .iter()
            .flat_map(UiBlock::owned_actions)
            .collect()
    }
    /// The inputs of `action`'s form, in form order.
    pub fn form_inputs(&self, action: &UiAction) -> Vec<&UiInput> {
        action
            .form
            .iter()
            .filter_map(|id| self.inputs.iter().find(|input| &input.id == id))
            .collect()
    }
    /// Validate the form values submitted for `action` (see [`validate_form_values`]).
    pub fn validate_form(
        &self,
        action: &UiAction,
        values: &Value,
    ) -> std::result::Result<Map<String, Value>, Vec<UiFormError>> {
        validate_form_values(&self.form_inputs(action), values)
    }
    /// [`Self::validate_form`] against the inputs as resolved over `data`, the
    /// data the document is shown with: `maxPath` and `defaultPath` read
    /// there, as in the form the host rendered. A composer-action form passes
    /// [`composer_data`].
    pub fn validate_form_over(
        &self,
        action: &UiAction,
        data: &Value,
        values: &Value,
    ) -> std::result::Result<Map<String, Value>, Vec<UiFormError>> {
        let inputs: Vec<UiInput> = self
            .form_inputs(action)
            .into_iter()
            .map(|input| input.resolve(data, None))
            .collect();
        validate_form_values(&inputs.iter().collect::<Vec<_>>(), values)
    }
    /// The admin-page filter inputs, in document order: those the rows block
    /// lists in `filters`, or every input when it declares no `filters`.
    pub fn filter_inputs(&self) -> Vec<&UiInput> {
        match self.rows().and_then(|rows| rows.filters.as_ref()) {
            Some(filters) => self
                .inputs
                .iter()
                .filter(|input| filters.contains(&input.id))
                .collect(),
            None => self.inputs.iter().collect(),
        }
    }
    /// Validate admin-page filter values: the [`Self::filter_inputs`] only. A
    /// value for any other input (a row action's form) is `unknown` here.
    pub fn validate_inputs(
        &self,
        values: &Value,
    ) -> std::result::Result<Map<String, Value>, Vec<UiFormError>> {
        validate_form_values(&self.filter_inputs(), values)
    }
    /// Drop the actions whose target `keep` rejects together with every
    /// reference to them, and a rows block whose source it rejects. The
    /// document stays valid, so a host can trim it to what a viewer may invoke.
    pub fn retain_targets(&mut self, keep: &mut dyn FnMut(&UiActionTarget) -> bool) {
        self.actions.retain(|action| keep(&action.target));
        let ids: BTreeSet<String> = self.actions.iter().map(|a| a.id.clone()).collect();
        self.blocks.retain_mut(|block| match block {
            UiBlock::Choices(choices) => {
                if choices.action.as_ref().is_some_and(|id| !ids.contains(id)) {
                    choices.action = None;
                }
                true
            }
            UiBlock::Rows(rows) => {
                rows.actions.retain(|id| ids.contains(id));
                keep(&UiActionTarget::Operation {
                    operation: rows.source.clone(),
                })
            }
            _ => true,
        });
        for translation in self.translations.values_mut() {
            translation.actions.retain(|id, _| ids.contains(id));
        }
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
    /// The client-visible subset of the tenant configuration
    /// ([`crate::PluginManifest::client_visible_configuration`]).
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub configuration: Map<String, Value>,
}
/// The data a composer-action document is resolved against, and its form
/// validated and its action input bound over: `{"config": configuration}`,
/// the plugin's client-visible configuration under [`COMPOSER_CONFIG_KEY`]
/// (`{"config": {}}` when there is none). So an input's `defaultPath` or
/// `maxPath` reads `/config/<key>`, and falls back to the declared value while
/// the tenant has not configured that key. No message payload or state layer
/// takes part.
pub fn composer_data(configuration: &Map<String, Value>) -> Value {
    let mut data = Map::new();
    data.insert(
        COMPOSER_CONFIG_KEY.into(),
        Value::Object(configuration.clone()),
    );
    Value::Object(data)
}
impl ClientPlugin {
    /// [`composer_data`] of this plugin's configuration.
    pub fn composer_data(&self) -> Value {
        composer_data(&self.configuration)
    }
    /// The data `contribution` (one of this plugin's) is resolved against:
    /// [`Self::composer_data`] for a composer-action, otherwise the payload
    /// under its state layers ([`UiStateLayers::merge`]).
    pub fn document_data(
        &self,
        contribution: &UiContribution,
        payload: &Value,
        layers: UiStateLayers<'_>,
    ) -> Value {
        if contribution.kind == UiContributionKind::ComposerAction {
            self.composer_data()
        } else {
            layers.merge(contribution, payload)
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCatalog {
    pub tenant_id: String,
    pub version: u64,
    pub valid_until_ms: i64,
    pub plugins: Vec<ClientPlugin>,
}
/// A plugin card in a conversation. Besides cards the host builds itself, an
/// operation result may carry one as `data.message` (see [`Self::from_result`]):
/// the host then sends it to the conversation the action ran in as a plugin
/// message, with a client message id that is stable for that action, and every
/// receiver renders it through the plugin's message-content contribution.
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
    /// The message a successful operation result of `plugin_id` asks the host to
    /// post: `data.message`, only when it is a valid message of that same plugin.
    /// Anything else (absent, malformed, another plugin's card) posts nothing.
    pub fn from_result(plugin_id: &str, data: &Value) -> Option<Self> {
        let message: Self = serde_json::from_value(data.get("message")?.clone()).ok()?;
        (message.plugin_id == plugin_id && message.validate().is_ok()).then_some(message)
    }
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
/// Presentation state a plugin returns for the message an action came from
/// (`data.messageState`). Clients keep the highest version per message and
/// overlay `values` on the payload before resolving: it is the local layer of
/// [`UiStateLayers`]. It grants nothing: every action is still authorized by
/// the plugin.
///
/// At most 16 keys `[A-Za-z0-9_]{1,64}` and 4 KiB; values are scalars (strings
/// up to 1024 bytes, such as the viewer's own entry text), or arrays of up to
/// 100 strings (each up to 256 bytes) or of up to 100 numbers (such as the
/// indices of `mySelection`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginMessageState {
    pub version: u64,
    pub values: Map<String, Value>,
}
impl PluginMessageState {
    pub const MAX_KEYS: usize = 16;
    pub const MAX_BYTES: usize = 4096;
    /// Bytes of a string value.
    pub const MAX_STRING_BYTES: usize = 1024;
    /// Bytes of a string inside an array value.
    pub const MAX_ITEM_BYTES: usize = 256;
    pub fn validate(&self) -> Result<()> {
        let text = |s: &str, max: usize| s.len() <= max && !s.chars().any(char::is_control);
        let valid_value = |value: &Value| match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => true,
            Value::String(s) => text(s, Self::MAX_STRING_BYTES),
            Value::Array(items) => {
                items.len() <= 100
                    && (items
                        .iter()
                        .all(|i| i.as_str().is_some_and(|s| text(s, Self::MAX_ITEM_BYTES)))
                        || items.iter().all(Value::is_number))
            }
            Value::Object(_) => false,
        };
        if self.values.len() > Self::MAX_KEYS
            || self
                .values
                .iter()
                .any(|(key, value)| !crate::state::valid_state_key(key) || !valid_value(value))
            || serde_json::to_vec(self).map_or(true, |v| v.len() > Self::MAX_BYTES)
        {
            return Err(ProtocolError::Denied("invalid plugin message state"));
        }
        Ok(())
    }
    /// Shallow overlay: a value replaces the top-level payload key, null removes it.
    pub fn merge_into(&self, payload: &Value) -> Value {
        crate::state::overlay(payload, &self.values)
    }
}
/// A resolved document, flat and ready to render: every value read, mapped and
/// translated, conditions applied. Kits render it without reading paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedPluginView {
    pub available: bool,
    pub title: String,
    pub fields: Vec<ResolvedUiField>,
    /// Visible actions not owned by a block, labels and confirmations translated.
    pub actions: Vec<UiAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<ResolvedUiBlock>,
    /// All document inputs, labels translated and initial values resolved; an
    /// action's `form` lists the ones it collects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<UiInput>,
    /// The contribution declares a detail ("view all") and its `hiddenWhen`
    /// does not match.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detail: bool,
    /// A detail view of a paged detail (`cursorPath`): the cursor of the next
    /// page, absent on the last page (see [`UiDetail::page_input`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}
impl ResolvedPluginView {
    /// Append the next page of a paged detail, resolved against the same
    /// detail document. Entries blocks pair up in order of appearance: the
    /// page's entries follow this view's in the same block, numbered on from
    /// this block's largest `index`, and `total` becomes the largest of the
    /// two totals and the merged count. Everything else (title, fields,
    /// actions, inputs, other blocks) stays as on the first page;
    /// `nextCursor` becomes the page's. An unavailable view or page ends
    /// paging without changing the entries.
    pub fn append_page(&mut self, page: ResolvedPluginView) {
        if !self.available || !page.available {
            self.next_cursor = None;
            return;
        }
        self.next_cursor = page.next_cursor;
        let mut pages = page.blocks.into_iter().filter_map(|block| match block {
            ResolvedUiBlock::Entries(entries) => Some(entries),
            _ => None,
        });
        for block in &mut self.blocks {
            let ResolvedUiBlock::Entries(entries) = block else {
                continue;
            };
            let Some(next) = pages.next() else {
                break;
            };
            let offset = entries
                .items
                .iter()
                .map(|item| item.index)
                .max()
                .unwrap_or(0);
            entries.items.extend(next.items.into_iter().map(|mut item| {
                item.index = item.index.saturating_add(offset);
                item
            }));
            entries.total = entries
                .total
                .max(next.total)
                .max(entries.items.len() as u64);
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedUiField {
    pub label: String,
    pub value: String,
}
/// Fields with a value, labels and values mapped by the given rules. A field
/// without a value is not shown: a card says what it knows, not an empty row.
pub(crate) fn resolve_fields(
    fields: &[UiField],
    data: &Value,
    label: impl Fn(&UiField, usize) -> String,
    value: impl Fn(&UiField, &str) -> Option<String>,
) -> Vec<ResolvedUiField> {
    fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            let raw = data.pointer(&field.path).and_then(scalar_text)?;
            if raw.trim().is_empty() {
                return None;
            }
            Some(ResolvedUiField {
                label: label(field, index),
                value: value(field, &raw).unwrap_or(raw),
            })
        })
        .collect()
}
pub(crate) fn resolve_action(action: &UiAction, translation: Option<&UiTranslation>) -> UiAction {
    let mut action = action.clone();
    action.hidden_when.clear();
    if let Some(label) = translation.and_then(|t| t.actions.get(&action.id)) {
        action.label = label.clone();
    }
    action.confirm = action.confirm.map(|text| translate(translation, &text));
    action
}
fn resolve_document(
    document: &UiDocument,
    data: &Value,
    locale: &str,
    viewer: &str,
    detail: Option<&UiDetail>,
) -> ResolvedPluginView {
    let translation = translation_for(&document.translations, locale);
    let fields = resolve_fields(
        &document.fields,
        data,
        |field, index| {
            translation
                .and_then(|t| t.fields.get(index))
                .unwrap_or(&field.label)
                .clone()
        },
        |field, raw| {
            translation
                .and_then(|t| t.values.get(&field.path))
                .and_then(|labels| labels.get(raw))
                .or_else(|| field.values.get(raw))
                .cloned()
        },
    );
    let owned = document.owned_actions();
    let actions = document
        .actions
        .iter()
        .filter(|action| {
            !owned.contains(action.id.as_str())
                && !action
                    .hidden_when
                    .iter()
                    .any(|condition| condition.matches(data, viewer))
        })
        .map(|action| resolve_action(action, translation))
        .collect();
    let detail = detail.filter(|d| !d.is_hidden(data, viewer));
    let scope = blocks::Scope {
        data,
        viewer,
        translation,
        actions: &document.actions,
        detail,
    };
    ResolvedPluginView {
        available: true,
        title: translation.map_or(&document.title, |t| &t.title).clone(),
        fields,
        actions,
        blocks: document
            .blocks
            .iter()
            .filter_map(|block| block.resolve(&scope))
            .collect(),
        inputs: document
            .inputs
            .iter()
            .map(|input| input.resolve(data, translation))
            .collect(),
        detail: detail.is_some(),
        next_cursor: None,
    }
}
impl UiContribution {
    /// Resolve the contribution's document against `data`: the merged card data
    /// ([`UiStateLayers::merge`]), a composer form's [`composer_data`], or an
    /// admin page's source result.
    pub fn resolve(&self, data: &Value, locale: &str, viewer: &str) -> ResolvedPluginView {
        resolve_document(&self.schema, data, locale, viewer, self.detail.as_ref())
    }
    /// Resolve the detail document against the detail operation's result data
    /// (one page of it, for a paged detail: the view then has `nextCursor`).
    pub fn resolve_detail(
        &self,
        data: &Value,
        locale: &str,
        viewer: &str,
    ) -> Option<ResolvedPluginView> {
        let detail = self.detail.as_ref()?;
        let mut view = resolve_document(&detail.document, data, locale, viewer, None);
        view.next_cursor = detail.next_cursor(data);
        Some(view)
    }
    /// [`UiDocument::retain_targets`] over the document and the detail; the
    /// detail is dropped when `keep` rejects its operation.
    pub fn retain_targets(&mut self, mut keep: impl FnMut(&UiActionTarget) -> bool) {
        self.schema.retain_targets(&mut keep);
        if self.detail.as_ref().is_some_and(|detail| {
            !keep(&UiActionTarget::Operation {
                operation: detail.operation.clone(),
            })
        }) {
            self.detail = None;
        }
        if let Some(detail) = &mut self.detail {
            detail.document.retain_targets(&mut keep);
        }
    }
}
fn unavailable(message: &PluginMessage) -> ResolvedPluginView {
    ResolvedPluginView {
        available: false,
        title: message
            .summary
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect(),
        fields: vec![],
        actions: vec![],
        blocks: vec![],
        inputs: vec![],
        detail: false,
        next_cursor: None,
    }
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
    /// [`Self::resolve_with_state`] without state layers: the payload as sent.
    pub fn resolve_with_context(
        &self,
        message: &PluginMessage,
        context: UiRenderContext<'_>,
    ) -> ResolvedPluginView {
        self.resolve_with_state(message, UiStateLayers::default(), context)
    }
    /// The view of a plugin message over its state layers
    /// (`payload ← shared ← local`); a composer-action form resolves against
    /// the plugin's [`composer_data`] instead ([`ClientPlugin::document_data`]).
    /// A message of another tenant, an expired catalog, an uninstalled plugin,
    /// another platform, an unknown contribution, version or kind, or an
    /// invalid document resolves to the inert summary with `available: false`.
    pub fn resolve_with_state(
        &self,
        message: &PluginMessage,
        state: UiStateLayers<'_>,
        context: UiRenderContext<'_>,
    ) -> ResolvedPluginView {
        match self.contribution_for(message, &context) {
            Some((plugin, contribution)) => contribution.resolve(
                &plugin.document_data(contribution, &message.payload, state),
                context.locale,
                context.viewer,
            ),
            None => unavailable(message),
        }
    }
    /// The detail view of a message, resolved against the result `data` of the
    /// detail operation ([`Self::detail`]).
    pub fn resolve_detail(
        &self,
        message: &PluginMessage,
        data: &Value,
        context: UiRenderContext<'_>,
    ) -> ResolvedPluginView {
        self.contribution_for(message, &context)
            .and_then(|(_, c)| c.resolve_detail(data, context.locale, context.viewer))
            .unwrap_or_else(|| unavailable(message))
    }
    fn contribution_for(
        &self,
        message: &PluginMessage,
        context: &UiRenderContext<'_>,
    ) -> Option<(&ClientPlugin, &UiContribution)> {
        if self.tenant_id != context.tenant
            || self.valid_until_ms <= context.now_ms
            || message.validate().is_err()
        {
            return None;
        }
        let plugin = self.plugins.iter().find(|p| {
            p.plugin_id == message.plugin_id && p.extensions.platforms.contains(&context.platform)
        })?;
        let contribution = plugin.extensions.contributions.iter().find(|c| {
            c.id == message.contribution_id
                && c.schema_version == message.schema_version
                && c.kind == context.kind
        })?;
        (contribution.schema.validate().is_ok()
            && contribution
                .detail
                .as_ref()
                .is_none_or(|detail| detail.validate().is_ok()))
        .then_some((plugin, contribution))
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
    /// The detail of a contribution, when its operation is in the catalog. Like
    /// [`Self::action`] this checks declaration and gesture only; Core
    /// authorizes the invocation. Hosts offer it only while the resolved view
    /// has `detail: true` (see [`UiDetail::hidden_when`]).
    pub fn detail(
        &self,
        tenant: &str,
        plugin_id: &str,
        contribution_id: &str,
        platform: ClientPlatform,
        user_gesture: bool,
        now_ms: i64,
    ) -> Result<UiDetail> {
        if tenant != self.tenant_id || now_ms >= self.valid_until_ms || !user_gesture {
            return Err(ProtocolError::Denied("UI detail unavailable"));
        }
        let p = self
            .plugins
            .iter()
            .find(|p| p.plugin_id == plugin_id && p.extensions.platforms.contains(&platform))
            .ok_or(ProtocolError::Denied("UI plugin unavailable"))?;
        let detail = p
            .extensions
            .contributions
            .iter()
            .find(|c| c.id == contribution_id && c.schema_version == 1)
            .and_then(|c| c.detail.as_ref())
            .ok_or(ProtocolError::Denied("UI detail unavailable"))?;
        detail.validate()?;
        if !p.operations.contains(&detail.operation) {
            return Err(ProtocolError::Denied("UI operation unavailable"));
        }
        Ok(detail.clone())
    }
}

fn bind_map(input: &BTreeMap<String, String>, payload: &Value) -> Result<Map<String, Value>> {
    input
        .iter()
        .map(|(name, path)| {
            let value = payload
                .pointer(path)
                .ok_or(ProtocolError::Denied("UI input unavailable"))?;
            Ok((name.clone(), value.clone()))
        })
        .collect()
}
fn bounded_input(output: Map<String, Value>) -> Result<Value> {
    let output = Value::Object(output);
    if serde_json::to_vec(&output).map_or(true, |v| v.len() > MAX_UI_ACTION_INPUT_BYTES) {
        return Err(ProtocolError::Denied("UI input limit"));
    }
    Ok(output)
}
pub(crate) fn bind_inputs(input: &BTreeMap<String, String>, payload: &Value) -> Result<Value> {
    bounded_input(bind_map(input, payload)?)
}

/// Copy only manifest-declared input fields. Historical data cannot inject
/// tenant/actor/permissions or extra parameters into a capability command.
pub fn bind_action_input(action: &UiAction, payload: &Value) -> Result<Value> {
    bind_inputs(&action.input, payload)
}

/// [`bind_action_input`] plus host-collected values: validated form values
/// ([`UiDocument::validate_form`]) or a choices selection
/// ([`UiChoicesBlock::selection_input`]). A name may not repeat a declared
/// input, so collected values never replace bound ones.
pub fn bind_action_input_with(
    action: &UiAction,
    payload: &Value,
    extra: Map<String, Value>,
) -> Result<Value> {
    let mut output = bind_map(&action.input, payload)?;
    for (name, value) in extra {
        if !valid_name(&name) || output.contains_key(&name) {
            return Err(ProtocolError::Denied("UI input collision"));
        }
        output.insert(name, value);
    }
    bounded_input(output)
}
