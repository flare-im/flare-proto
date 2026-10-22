//! Short-lived, target-bound plugin tickets. Deliberately not an IM signed
//! context or a platform control assertion: other IM/Social APIs cannot accept
//! this ticket as a user token. Signing keys must be independent of Cell keys.
//!
//! Three credentials share one envelope format and differ in their signing
//! domain, so none verifies as another:
//! - [`PluginCallClaims`]: Core → plugin, a user invoked an operation;
//! - [`PluginStaffCallClaims`]: Core → plugin, a tenant staff member invoked an
//!   admin-page operation from the console (`dispatch_admin`);
//! - [`PluginRuntimeClaims`]: plugin → Core, the plugin deployment itself calls a
//!   host write capability with no user or staff call in flight.
use crate::{ProtocolError, Result, resource::HostCapability};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;
pub const PLUGIN_CALL_HEADER: &str = "x-flare-plugin-call";
/// Header of the [`PluginRuntimeClaims`] credential on a resource-bridge call.
pub const PLUGIN_RUNTIME_HEADER: &str = "x-flare-plugin-runtime";
pub const MAX_CALL_TTL_MS: i64 = 120_000;
pub const MAX_RUNTIME_CALL_TTL_MS: i64 = 60_000;
/// Permission codes are at most 128 bytes across the signed context, the
/// platform permission catalog and audit, so an admin-page plugin id is at most
/// 115 bytes (`plugin:` + id + `:admin`).
pub const MAX_PERMISSION_CODE_BYTES: usize = 128;

const CALL_DOMAIN: &str = "flare.plugin.call.v1";
const STAFF_CALL_DOMAIN: &str = "flare.plugin.staff-call.v1";
const RUNTIME_DOMAIN: &str = "flare.plugin.runtime.v1";

/// The staff permission code that authorizes the admin operations of
/// `plugin_id`: `plugin:<pluginId>:admin`. `None` when it would exceed
/// [`MAX_PERMISSION_CODE_BYTES`].
pub fn plugin_admin_permission(plugin_id: &str) -> Option<String> {
    let code = format!("plugin:{plugin_id}:admin");
    (!plugin_id.is_empty() && code.len() <= MAX_PERMISSION_CODE_BYTES).then_some(code)
}

/// The plugin id of a `plugin:<pluginId>:admin` permission code.
pub fn plugin_admin_target(permission: &str) -> Option<&str> {
    let id = permission.strip_prefix("plugin:")?.strip_suffix(":admin")?;
    (!id.is_empty() && permission.len() <= MAX_PERMISSION_CODE_BYTES && !id.contains(':'))
        .then_some(id)
}

fn bounded_ids(values: &[&str]) -> bool {
    values
        .iter()
        .all(|v| !v.is_empty() && v.len() <= 192 && !v.chars().any(char::is_control))
}

fn lifetime_ok(issued_at_ms: i64, expires_at_ms: i64, now_ms: i64, max_ttl_ms: i64) -> bool {
    issued_at_ms >= 0
        && issued_at_ms <= now_ms.saturating_add(30_000)
        && expires_at_ms > now_ms
        && expires_at_ms > issued_at_ms
        && expires_at_ms.saturating_sub(issued_at_ms) <= max_ttl_ms
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCallClaims {
    pub tenant_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_digest: String,
    pub deployment_id: String,
    pub grant_id: String,
    pub grant_revision: u64,
    pub configuration_digest: String,
    pub actor_id: String,
    pub operation: String,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
}
impl PluginCallClaims {
    pub fn validate(&self, now_ms: i64) -> Result<()> {
        if !bounded_ids(&[
            &self.tenant_id,
            &self.plugin_id,
            &self.plugin_version,
            &self.deployment_id,
            &self.grant_id,
            &self.actor_id,
            &self.operation,
            &self.request_id,
        ]) || self.tenant_id == "*"
            || self.grant_revision == 0
            || !lifetime_ok(
                self.issued_at_ms,
                self.expires_at_ms,
                now_ms,
                MAX_CALL_TTL_MS,
            )
            || !crate::manifest::is_sha256_digest(&self.manifest_digest)
            || !crate::manifest::is_sha256_digest(&self.configuration_digest)
            || self
                .conversation_id
                .as_ref()
                .is_some_and(|id| !crate::resource::valid_resource_id(id))
        {
            return Err(ProtocolError::Denied("invalid plugin invocation ticket"));
        }
        Ok(())
    }
}

/// Ticket for an admin-page operation invoked by a tenant staff member. Core
/// signs it only after it verified a `Staff` signed context for exactly this
/// tenant whose permission is [`plugin_admin_permission`] of this plugin, and
/// only for an operation in [`crate::PluginManifest::admin_operations`]. The
/// plugin verifies it with [`PluginCallVerifier::verify_staff`] and still
/// applies its own rules. It carries no conversation: admin work is
/// tenant-scoped and names its entity in the input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginStaffCallClaims {
    pub tenant_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_digest: String,
    pub deployment_id: String,
    pub grant_id: String,
    pub grant_revision: u64,
    pub configuration_digest: String,
    pub staff_id: String,
    /// The staff member's organization; `None` for platform staff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    /// Always `plugin:<pluginId>:admin`.
    pub permission: String,
    pub operation: String,
    pub request_id: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
}
impl PluginStaffCallClaims {
    pub fn validate(&self, now_ms: i64) -> Result<()> {
        if !bounded_ids(&[
            &self.tenant_id,
            &self.plugin_id,
            &self.plugin_version,
            &self.deployment_id,
            &self.grant_id,
            &self.staff_id,
            &self.operation,
            &self.request_id,
        ]) || self.tenant_id == "*"
            || self.grant_revision == 0
            || self.org_id.as_deref().is_some_and(|id| !bounded_ids(&[id]))
            || plugin_admin_permission(&self.plugin_id).as_deref() != Some(self.permission.as_str())
            || !lifetime_ok(
                self.issued_at_ms,
                self.expires_at_ms,
                now_ms,
                MAX_CALL_TTL_MS,
            )
            || !crate::manifest::is_sha256_digest(&self.manifest_digest)
            || !crate::manifest::is_sha256_digest(&self.configuration_digest)
        {
            return Err(ProtocolError::Denied("invalid plugin staff ticket"));
        }
        Ok(())
    }
}

/// Credential the plugin deployment signs with its own runtime key for a host
/// write when no user or staff call is in flight (a deadline closing a poll).
/// It authorizes one capability for one exact request body: Core recomputes
/// `requestDigest` over the bytes it received. Core maps the key id to the
/// deployment it was provisioned for; the grant must still be enabled at that
/// revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRuntimeClaims {
    pub tenant_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_digest: String,
    pub deployment_id: String,
    pub grant_id: String,
    pub grant_revision: u64,
    /// A write capability ([`HostCapability::is_write`]).
    pub capability: HostCapability,
    pub request_id: String,
    /// `sha256:<hex>` of the exact resource request bytes.
    pub request_digest: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
}
impl PluginRuntimeClaims {
    pub fn validate(&self, now_ms: i64) -> Result<()> {
        if !bounded_ids(&[
            &self.tenant_id,
            &self.plugin_id,
            &self.plugin_version,
            &self.deployment_id,
            &self.grant_id,
            &self.request_id,
        ]) || self.tenant_id == "*"
            || self.grant_revision == 0
            || !self.capability.is_write()
            || !lifetime_ok(
                self.issued_at_ms,
                self.expires_at_ms,
                now_ms,
                MAX_RUNTIME_CALL_TTL_MS,
            )
            || !crate::manifest::is_sha256_digest(&self.manifest_digest)
            || !crate::manifest::is_sha256_digest(&self.request_digest)
        {
            return Err(ProtocolError::Denied("invalid plugin runtime credential"));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    key_id: String,
    payload: String,
    signature: String,
}
fn seal<T: Serialize>(domain: &str, key_id: &str, key: &SigningKey, claims: &T) -> Result<String> {
    let payload =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).map_err(|_| ProtocolError::InvalidJson)?);
    let signature = URL_SAFE_NO_PAD.encode(key.sign(&message(domain, key_id, &payload)).to_bytes());
    Ok(URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&Envelope {
            key_id: key_id.to_owned(),
            payload,
            signature,
        })
        .map_err(|_| ProtocolError::InvalidJson)?,
    ))
}
/// Verify the envelope under `domain` with the key `lookup` returns for its key
/// id, and decode the claims. Returns the key id with the claims.
fn open<'k, T: DeserializeOwned>(
    domain: &str,
    token: &str,
    lookup: impl FnOnce(&str) -> Option<&'k VerifyingKey>,
) -> Result<(String, T)> {
    if token.len() > 16384 {
        return Err(ProtocolError::InvalidSignature);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| ProtocolError::InvalidSignature)?;
    let envelope: Envelope =
        serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidSignature)?;
    let key = lookup(&envelope.key_id).ok_or(ProtocolError::InvalidSignature)?;
    let signature = Signature::from_slice(
        &URL_SAFE_NO_PAD
            .decode(&envelope.signature)
            .map_err(|_| ProtocolError::InvalidSignature)?,
    )
    .map_err(|_| ProtocolError::InvalidSignature)?;
    key.verify_strict(
        &message(domain, &envelope.key_id, &envelope.payload),
        &signature,
    )
    .map_err(|_| ProtocolError::InvalidSignature)?;
    let claims = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(&envelope.payload)
            .map_err(|_| ProtocolError::InvalidSignature)?,
    )
    .map_err(|_| ProtocolError::InvalidSignature)?;
    Ok((envelope.key_id, claims))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicKeyEntry {
    key_id: String,
    #[serde(default)]
    deployment_id: Option<String>,
    public_key_base64: String,
}
fn public_keys(raw: &str) -> Result<Vec<(PublicKeyEntry, [u8; 32])>> {
    if raw.len() > 32768 {
        return Err(ProtocolError::InvalidSignature);
    }
    let keys: Vec<PublicKeyEntry> =
        serde_json::from_str(raw).map_err(|_| ProtocolError::InvalidSignature)?;
    if keys.is_empty() || keys.len() > 128 {
        return Err(ProtocolError::InvalidSignature);
    }
    keys.into_iter()
        .map(|key| {
            let public = base64::engine::general_purpose::STANDARD
                .decode(&key.public_key_base64)
                .ok()
                .and_then(|v| v.try_into().ok())
                .ok_or(ProtocolError::InvalidSignature)?;
            Ok((key, public))
        })
        .collect()
}

pub struct PluginCallSigner {
    key_id: String,
    key: SigningKey,
}
impl PluginCallSigner {
    pub fn new(key_id: String, seed: [u8; 32]) -> Result<Self> {
        valid_key_id(&key_id)?;
        Ok(Self {
            key_id,
            key: SigningKey::from_bytes(&seed),
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
    pub fn sign(&self, claims: &PluginCallClaims, now_ms: i64) -> Result<String> {
        claims.validate(now_ms)?;
        seal(CALL_DOMAIN, &self.key_id, &self.key, claims)
    }
    pub fn sign_staff(&self, claims: &PluginStaffCallClaims, now_ms: i64) -> Result<String> {
        claims.validate(now_ms)?;
        seal(STAFF_CALL_DOMAIN, &self.key_id, &self.key, claims)
    }
}
#[derive(Default, Clone)]
pub struct PluginCallVerifier {
    keys: BTreeMap<String, VerifyingKey>,
}
impl PluginCallVerifier {
    /// `[{"key_id": "...", "public_key_base64": "..."}]`, at most 128 keys.
    pub fn from_public_keys_json(raw: &str) -> Result<Self> {
        let mut verifier = Self::default();
        for (key, public) in public_keys(raw)? {
            if key.deployment_id.is_some() {
                return Err(ProtocolError::InvalidSignature);
            }
            verifier.insert(key.key_id, public)?;
        }
        Ok(verifier)
    }
    pub fn insert(&mut self, id: String, key: [u8; 32]) -> Result<()> {
        valid_key_id(&id)?;
        let key = VerifyingKey::from_bytes(&key).map_err(|_| ProtocolError::InvalidSignature)?;
        if (self.keys.len() >= 128 && !self.keys.contains_key(&id))
            || self.keys.get(&id).is_some_and(|old| old != &key)
        {
            return Err(ProtocolError::InvalidSignature);
        }
        self.keys.insert(id, key);
        Ok(())
    }
    /// A user ticket. A staff ticket never verifies here.
    pub fn verify(&self, token: &str, now_ms: i64) -> Result<PluginCallClaims> {
        let (_, claims): (_, PluginCallClaims) = open(CALL_DOMAIN, token, |id| self.keys.get(id))?;
        claims.validate(now_ms)?;
        Ok(claims)
    }
    /// A staff ticket. A user ticket never verifies here. The caller still
    /// checks plugin, deployment, operation (one of the manifest's
    /// `admin_operations`), request id and the tenant grant revision, exactly as
    /// for a user ticket.
    pub fn verify_staff(&self, token: &str, now_ms: i64) -> Result<PluginStaffCallClaims> {
        let (_, claims): (_, PluginStaffCallClaims) =
            open(STAFF_CALL_DOMAIN, token, |id| self.keys.get(id))?;
        claims.validate(now_ms)?;
        Ok(claims)
    }
}

/// Held by the plugin deployment; the key is provisioned per deployment and is
/// independent of publisher, Cell and call-signing keys.
pub struct PluginRuntimeSigner {
    key_id: String,
    key: SigningKey,
}
impl PluginRuntimeSigner {
    pub fn new(key_id: String, seed: [u8; 32]) -> Result<Self> {
        valid_key_id(&key_id)?;
        Ok(Self {
            key_id,
            key: SigningKey::from_bytes(&seed),
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
    pub fn sign(&self, claims: &PluginRuntimeClaims, now_ms: i64) -> Result<String> {
        claims.validate(now_ms)?;
        seal(RUNTIME_DOMAIN, &self.key_id, &self.key, claims)
    }
}
/// Core side: each key verifies only credentials of the deployment it was
/// provisioned for.
#[derive(Default, Clone)]
pub struct PluginRuntimeVerifier {
    keys: BTreeMap<String, (String, VerifyingKey)>,
}
impl PluginRuntimeVerifier {
    /// `[{"key_id": "...", "deployment_id": "...", "public_key_base64": "..."}]`,
    /// at most 128 keys.
    pub fn from_public_keys_json(raw: &str) -> Result<Self> {
        let mut verifier = Self::default();
        for (key, public) in public_keys(raw)? {
            let deployment = key.deployment_id.ok_or(ProtocolError::InvalidSignature)?;
            verifier.insert(key.key_id, deployment, public)?;
        }
        Ok(verifier)
    }
    pub fn insert(&mut self, id: String, deployment_id: String, key: [u8; 32]) -> Result<()> {
        valid_key_id(&id)?;
        if !bounded_ids(&[&deployment_id]) || deployment_id.len() > 128 {
            return Err(ProtocolError::InvalidSignature);
        }
        let key = VerifyingKey::from_bytes(&key).map_err(|_| ProtocolError::InvalidSignature)?;
        if (self.keys.len() >= 128 && !self.keys.contains_key(&id))
            || self
                .keys
                .get(&id)
                .is_some_and(|old| old != &(deployment_id.clone(), key))
        {
            return Err(ProtocolError::InvalidSignature);
        }
        self.keys.insert(id, (deployment_id, key));
        Ok(())
    }
    /// The credential for exactly `request` (the resource request bytes).
    pub fn verify(&self, token: &str, request: &[u8], now_ms: i64) -> Result<PluginRuntimeClaims> {
        let mut deployment = None;
        let (_, claims): (_, PluginRuntimeClaims) = open(RUNTIME_DOMAIN, token, |id| {
            self.keys.get(id).map(|(owner, key)| {
                deployment = Some(owner.clone());
                key
            })
        })?;
        claims.validate(now_ms)?;
        if deployment.as_deref() != Some(claims.deployment_id.as_str())
            || claims.request_digest != crate::runtime::digest_bytes(request)
        {
            return Err(ProtocolError::InvalidSignature);
        }
        Ok(claims)
    }
}
fn valid_key_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(ProtocolError::InvalidSignature);
    }
    Ok(())
}
fn message(domain: &str, id: &str, payload: &str) -> Vec<u8> {
    format!("{domain}\n{id}\n{payload}").into_bytes()
}
