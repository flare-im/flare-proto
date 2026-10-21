use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ProtocolError, Result};

pub const MANIFEST_VERSION: u32 = 1;
pub const PLUGIN_API_VERSION: &str = "1.0";
pub const MAX_MANIFEST_BYTES: usize = 512 * 1024;

/// Canonical platform contract. Endpoint addresses, secrets and tenant state are
/// supplied by the host; they must never be embedded in a published manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub manifest_version: u32,
    pub plugin_id: String,
    pub version: String,
    pub api_version: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub publisher: Publisher,
    pub runtime: PluginRuntime,
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    pub capabilities: Vec<Capability>,
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub events: Vec<PluginEvent>,
    #[serde(default)]
    pub hooks: Vec<PluginHook>,
    #[serde(default)]
    pub client_extensions: ClientExtensions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_schema: Option<Value>,
    #[serde(default)]
    pub data_policy: DataPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<PluginArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_version_range: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Publisher {
    pub publisher_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum PluginRuntime {
    Grpc {
        service: String,
        protocol: String,
    },
    Wasm {
        module: String,
        #[serde(rename = "abiVersion")]
        abi_version: String,
        #[serde(rename = "maxMemoryPages")]
        max_memory_pages: u32,
        #[serde(rename = "maxFuel")]
        max_fuel: u64,
    },
    Client,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub version_range: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub id: String,
    pub operations: Vec<PluginOperation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginOperation {
    /// Relative to the capability id; the wire operation is `<capability>.<id>`.
    pub id: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub permission_ids: Vec<String>,
    pub idempotent: bool,
    pub timeout_ms: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_capabilities: Vec<crate::resource::HostCapability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPermission {
    pub id: String,
    pub description: String,
    pub data_scopes: Vec<String>,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginEvent {
    pub id: String,
    pub schema_version: u32,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginHook {
    pub id: String,
    pub api_version: String,
    pub timeout_ms: u32,
    /// A request to the host. The host owns the permitted hook failure policy.
    pub failure_mode: HookFailureMode,
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HookFailureMode {
    Deny,
    Skip,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientExtensions {
    #[serde(default)]
    pub platforms: Vec<ClientPlatform>,
    #[serde(default)]
    pub contributions: Vec<UiContribution>,
    #[serde(default)]
    pub native_capabilities: Vec<crate::ui::NativeCapability>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ClientPlatform {
    Web,
    Tauri,
    Android,
    Ios,
    Flutter,
    Harmonyos,
}

/// Kit icon registry names: kebab-case, starting with a letter (`red-packet`).
pub const UI_ICON_PATTERN: &str = r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$";
pub const UI_ICON_MAX_LEN: usize = 32;
/// Order of a contribution that declares none.
pub const DEFAULT_UI_ORDER: i32 = 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiContribution {
    pub kind: UiContributionKind,
    pub id: String,
    pub schema_version: u32,
    pub schema: crate::ui::UiDocument,
    pub required_permissions: Vec<String>,
    /// Name in the design kit's icon registry. Presentation only: a kit that
    /// does not know the name shows its generic plugin icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = UI_ICON_MAX_LEN), regex(pattern = UI_ICON_PATTERN))]
    pub icon: Option<String>,
    /// Ascending position among entries of the same kind (default 1000). Hosts
    /// list plugin entries after their built-in ones; ties sort by title.
    #[serde(
        default = "default_ui_order",
        skip_serializing_if = "is_default_ui_order"
    )]
    #[schemars(extend("default" = DEFAULT_UI_ORDER))]
    pub order: i32,
}

fn default_ui_order() -> i32 {
    DEFAULT_UI_ORDER
}

fn is_default_ui_order(order: &i32) -> bool {
    *order == DEFAULT_UI_ORDER
}

/// Whether `value` is a well-formed kit icon name (see [`UI_ICON_PATTERN`]).
pub fn is_ui_icon_name(value: &str) -> bool {
    value.len() <= UI_ICON_MAX_LEN
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UiContributionKind {
    MessageContent,
    MessageAction,
    ConversationMenu,
    ComposerAction,
    Page,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataPolicy {
    #[serde(default)]
    pub tenant_isolation: TenantIsolation,
    #[serde(default)]
    pub uninstall_behavior: UninstallBehavior,
    #[serde(default)]
    pub migration_policy: MigrationPolicy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TenantIsolation {
    #[default]
    Required,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UninstallBehavior {
    #[default]
    Retain,
    ExportThenRetain,
    DeleteByRequest,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationPolicy {
    #[default]
    None,
    BackwardCompatible,
    Versioned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginArtifact {
    pub digest: String,
    pub signature_ref: String,
}

impl PluginManifest {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ProtocolError::ManifestTooLarge(MAX_MANIFEST_BYTES));
        }
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|_| ProtocolError::InvalidJson)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        ensure(
            self.manifest_version == MANIFEST_VERSION,
            "manifestVersion",
            "unsupported version",
        )?;
        ensure(
            self.api_version == PLUGIN_API_VERSION,
            "apiVersion",
            "unsupported API version",
        )?;
        identifier(&self.plugin_id, "pluginId")?;
        ensure(self.plugin_id.len() <= 128, "pluginId", "too long")?;
        ensure(
            self.version.len() <= 64 && semver::Version::parse(&self.version).is_ok(),
            "version",
            "expected semantic version",
        )?;
        bounded(&self.display_name, 160, "displayName")?;
        ensure(self.description.len() <= 4000, "description", "too long")?;
        identifier(&self.publisher.publisher_id, "publisher.publisherId")?;
        bounded(&self.publisher.display_name, 160, "publisher.displayName")?;
        if let Some(range) = &self.host_version_range {
            version_range(range, "hostVersionRange")?;
        }
        match &self.runtime {
            PluginRuntime::Grpc { service, protocol } => {
                bounded(service, 256, "runtime.service")?;
                ensure(
                    protocol == "flare.capability.v1",
                    "runtime.protocol",
                    "unsupported protocol",
                )?;
            }
            PluginRuntime::Wasm {
                module,
                abi_version,
                max_memory_pages,
                max_fuel,
            } => {
                bounded(module, 512, "runtime.module")?;
                ensure(
                    !module.starts_with('/')
                        && !module.contains('\\')
                        && module
                            .split('/')
                            .all(|part| !part.is_empty() && part != "." && part != ".."),
                    "runtime.module",
                    "expected relative artifact path",
                )?;
                bounded(abi_version, 32, "runtime.abiVersion")?;
                ensure(
                    (1..=65536).contains(max_memory_pages),
                    "runtime.maxMemoryPages",
                    "out of range",
                )?;
                ensure(*max_fuel > 0, "runtime.maxFuel", "must be positive")?;
            }
            PluginRuntime::Client => {}
        }
        if let Some(artifact) = &self.artifact {
            ensure(
                is_sha256_digest(&artifact.digest),
                "artifact.digest",
                "expected sha256 digest",
            )?;
            bounded(&artifact.signature_ref, 512, "artifact.signatureRef")?;
        } else {
            ensure(
                matches!(self.runtime, PluginRuntime::Client),
                "artifact",
                "server runtime requires signed artifact reference",
            )?;
        }
        ensure(
            self.dependencies.len() <= 64,
            "dependencies",
            "too many dependencies",
        )?;
        let mut dependencies = BTreeSet::new();
        for dep in &self.dependencies {
            identifier(&dep.plugin_id, "dependencies.pluginId")?;
            ensure(
                dep.plugin_id != self.plugin_id && dependencies.insert(&dep.plugin_id),
                "dependencies",
                "duplicate or self dependency",
            )?;
            version_range(&dep.version_range, "dependencies.versionRange")?;
        }
        let mut permissions = BTreeSet::new();
        for permission in &self.permissions {
            identifier(&permission.id, "permissions.id")?;
            ensure(
                permissions.insert(permission.id.as_str()),
                "permissions",
                "duplicate permission",
            )?;
            bounded(&permission.description, 1000, "permissions.description")?;
            unique_identifiers(&permission.data_scopes, "permissions.dataScopes")?;
        }
        ensure(
            !self.capabilities.is_empty() && self.capabilities.len() <= 128,
            "capabilities",
            "expected 1..128 capabilities",
        )?;
        let mut capabilities = BTreeSet::new();
        let mut operation_ids = BTreeSet::new();
        for cap in &self.capabilities {
            identifier(&cap.id, "capabilities.id")?;
            ensure(
                capabilities.insert(&cap.id),
                "capabilities",
                "duplicate capability",
            )?;
            ensure(
                !cap.operations.is_empty() && cap.operations.len() <= 256,
                "capabilities.operations",
                "expected 1..256 operations",
            )?;
            for op in &cap.operations {
                identifier(&op.id, "operations.id")?;
                ensure(
                    operation_ids.insert(format!("{}.{}", cap.id, op.id)),
                    "operations",
                    "duplicate wire operation",
                )?;
                ensure(
                    cap.id.len() + 1 + op.id.len() <= 128,
                    "operations.id",
                    "wire operation exceeds 128 bytes",
                )?;
                schema(&op.input_schema, "operations.inputSchema")?;
                schema(&op.output_schema, "operations.outputSchema")?;
                permission_refs(&op.permission_ids, &permissions, "operations.permissionIds")?;
                let mut host_capabilities = BTreeSet::new();
                for capability in &op.host_capabilities {
                    ensure(
                        host_capabilities.insert(capability),
                        "operations.hostCapabilities",
                        "duplicate host capability",
                    )?;
                    ensure(
                        self.permissions.iter().any(|permission| {
                            op.permission_ids.contains(&permission.id)
                                && permission
                                    .data_scopes
                                    .iter()
                                    .any(|scope| scope == capability.id())
                        }),
                        "operations.hostCapabilities",
                        "host capability requires an explicit operation permission data scope",
                    )?;
                }
                ensure(
                    (1..=120000).contains(&op.timeout_ms),
                    "operations.timeoutMs",
                    "out of range",
                )?;
            }
        }
        ensure(
            operation_ids.len() <= 256,
            "capabilities.operations",
            "expected at most 256 total wire operations",
        )?;
        let mut event_ids = BTreeSet::new();
        for event in &self.events {
            identifier(&event.id, "events.id")?;
            ensure(event_ids.insert(&event.id), "events", "duplicate event")?;
            ensure(
                event.schema_version > 0,
                "events.schemaVersion",
                "must be positive",
            )?;
            schema(&event.schema, "events.schema")?;
        }
        let mut hook_ids = BTreeSet::new();
        for hook in &self.hooks {
            identifier(&hook.id, "hooks.id")?;
            ensure(hook_ids.insert(&hook.id), "hooks", "duplicate hook")?;
            ensure(
                hook.api_version == PLUGIN_API_VERSION,
                "hooks.apiVersion",
                "unsupported API version",
            )?;
            ensure(
                (1..=5000).contains(&hook.timeout_ms),
                "hooks.timeoutMs",
                "out of range",
            )?;
            permission_refs(&hook.permission_ids, &permissions, "hooks.permissionIds")?;
        }
        if let Some(configuration) = &self.configuration_schema {
            schema(configuration, "configurationSchema")?;
        }
        let mut contribution_ids = BTreeSet::new();
        for contribution in &self.client_extensions.contributions {
            identifier(&contribution.id, "clientExtensions.contributions.id")?;
            ensure(
                contribution_ids.insert(&contribution.id),
                "clientExtensions.contributions",
                "duplicate contribution",
            )?;
            ensure(
                contribution.schema_version == 1,
                "clientExtensions.schemaVersion",
                "must be positive",
            )?;
            contribution.schema.validate()?;
            ensure(
                contribution.icon.as_deref().is_none_or(is_ui_icon_name),
                "clientExtensions.contributions.icon",
                "expected kebab-case kit icon name of at most 32 bytes",
            )?;
            ensure(
                contribution.id.starts_with(&format!("{}.", self.plugin_id)),
                "clientExtensions.id",
                "must belong to plugin",
            )?;
            for action in &contribution.schema.actions {
                use crate::ui::UiActionTarget;
                let declared = match &action.target {
                    UiActionTarget::Operation { operation } => self.operation(operation).is_some(),
                    UiActionTarget::Page { page } => self
                        .client_extensions
                        .contributions
                        .iter()
                        .any(|c| &c.id == page && c.kind == UiContributionKind::Page),
                    UiActionTarget::Native { capability } => self
                        .client_extensions
                        .native_capabilities
                        .contains(capability),
                };
                ensure(
                    declared,
                    "clientExtensions.actions",
                    "target must be declared",
                )?;
            }
            permission_refs(
                &contribution.required_permissions,
                &permissions,
                "clientExtensions.requiredPermissions",
            )?;
        }
        ensure(
            self.client_extensions
                .platforms
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == self.client_extensions.platforms.len(),
            "clientExtensions.platforms",
            "duplicate platform",
        )?;
        ensure(
            self.client_extensions
                .native_capabilities
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == self.client_extensions.native_capabilities.len(),
            "clientExtensions.nativeCapabilities",
            "duplicate capability",
        )?;
        ensure(
            self.client_extensions.contributions.len() <= 64,
            "clientExtensions.contributions",
            "too many contributions",
        )?;
        Ok(())
    }

    pub fn operation(&self, wire_id: &str) -> Option<&PluginOperation> {
        self.capabilities.iter().find_map(|cap| {
            wire_id
                .strip_prefix(&format!("{}.", cap.id))
                .and_then(|id| cap.operations.iter().find(|op| op.id == id))
        })
    }

    pub fn operation_ids(&self) -> BTreeSet<String> {
        self.capabilities
            .iter()
            .flat_map(|cap| {
                cap.operations
                    .iter()
                    .map(|op| format!("{}.{}", cap.id, op.id))
            })
            .collect()
    }
}

pub fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn identifier(value: &str, field: &str) -> Result<()> {
    ensure(
        !value.is_empty()
            && value.len() <= 192
            && value.split('.').all(|part| {
                part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                    && part.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
                    })
            }),
        field,
        "expected bounded lowercase namespaced identifier",
    )
}

fn bounded(value: &str, max: usize, field: &str) -> Result<()> {
    ensure(
        !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control),
        field,
        "empty, control character or too long",
    )
}

fn version_range(range: &str, field: &str) -> Result<()> {
    ensure(
        range.len() <= 128 && semver::VersionReq::parse(range).is_ok(),
        field,
        "invalid semantic version range",
    )
}

fn unique_identifiers(values: &[String], field: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values {
        identifier(value, field)?;
        ensure(seen.insert(value), field, "duplicate identifier")?;
    }
    Ok(())
}

fn permission_refs(values: &[String], declared: &BTreeSet<&str>, field: &str) -> Result<()> {
    unique_identifiers(values, field)?;
    ensure(
        values.iter().all(|id| declared.contains(id.as_str())),
        field,
        "undeclared permission",
    )
}

fn schema(value: &Value, field: &str) -> Result<()> {
    ensure(
        value.is_object()
            && (value.get("type").is_some()
                || value.get("$ref").is_some()
                || value.get("oneOf").is_some()),
        field,
        "expected typed JSON schema",
    )?;
    fn local_refs(value: &Value, field: &str, budget: &mut usize) -> Result<()> {
        ensure(*budget > 0, field, "schema complexity limit exceeded")?;
        *budget -= 1;
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    if key == "$ref" || key == "$dynamicRef" {
                        ensure(
                            child
                                .as_str()
                                .is_some_and(|reference| reference.starts_with('#')),
                            field,
                            "external schema references are forbidden",
                        )?;
                    }
                    local_refs(child, field, budget)?;
                }
            }
            Value::Array(array) => {
                for child in array {
                    local_refs(child, field, budget)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    local_refs(value, field, &mut 8192)
}

fn ensure(condition: bool, field: &str, reason: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(ProtocolError::InvalidManifest {
            field: field.to_owned(),
            reason: reason.to_owned(),
        })
    }
}
