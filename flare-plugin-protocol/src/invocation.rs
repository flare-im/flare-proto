//! Short-lived, target-bound plugin tickets. Deliberately not an IM signed
//! context or a platform control assertion: other IM/Social APIs cannot accept
//! this ticket as a user token. Signing keys must be independent of Cell keys.
use crate::{ProtocolError, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const PLUGIN_CALL_HEADER: &str = "x-flare-plugin-call";
pub const MAX_CALL_TTL_MS: i64 = 120_000;

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
        if [
            &self.tenant_id,
            &self.plugin_id,
            &self.plugin_version,
            &self.deployment_id,
            &self.grant_id,
            &self.actor_id,
            &self.operation,
            &self.request_id,
        ]
        .iter()
        .any(|v| v.is_empty() || v.len() > 192 || v.chars().any(char::is_control))
            || self.tenant_id == "*"
            || self.grant_revision == 0
            || self.issued_at_ms < 0
            || self.issued_at_ms > now_ms.saturating_add(30_000)
            || self.expires_at_ms <= now_ms
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms.saturating_sub(self.issued_at_ms) > MAX_CALL_TTL_MS
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
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    key_id: String,
    payload: String,
    signature: String,
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
        let payload = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(claims).map_err(|_| ProtocolError::InvalidJson)?);
        let signature =
            URL_SAFE_NO_PAD.encode(self.key.sign(&message(&self.key_id, &payload)).to_bytes());
        Ok(URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&Envelope {
                key_id: self.key_id.clone(),
                payload,
                signature,
            })
            .map_err(|_| ProtocolError::InvalidJson)?,
        ))
    }
}
#[derive(Default, Clone)]
pub struct PluginCallVerifier {
    keys: BTreeMap<String, VerifyingKey>,
}
impl PluginCallVerifier {
    pub fn from_public_keys_json(raw: &str) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Key {
            key_id: String,
            public_key_base64: String,
        }
        if raw.len() > 32768 {
            return Err(ProtocolError::InvalidSignature);
        }
        let keys: Vec<Key> =
            serde_json::from_str(raw).map_err(|_| ProtocolError::InvalidSignature)?;
        if keys.is_empty() || keys.len() > 128 {
            return Err(ProtocolError::InvalidSignature);
        }
        let mut verifier = Self::default();
        for key in keys {
            let public = base64::engine::general_purpose::STANDARD
                .decode(key.public_key_base64)
                .ok()
                .and_then(|v| v.try_into().ok())
                .ok_or(ProtocolError::InvalidSignature)?;
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
    pub fn verify(&self, token: &str, now_ms: i64) -> Result<PluginCallClaims> {
        if token.len() > 16384 {
            return Err(ProtocolError::InvalidSignature);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidSignature)?;
        let key = self
            .keys
            .get(&envelope.key_id)
            .ok_or(ProtocolError::InvalidSignature)?;
        let signature = Signature::from_slice(
            &URL_SAFE_NO_PAD
                .decode(&envelope.signature)
                .map_err(|_| ProtocolError::InvalidSignature)?,
        )
        .map_err(|_| ProtocolError::InvalidSignature)?;
        key.verify_strict(&message(&envelope.key_id, &envelope.payload), &signature)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        let claims: PluginCallClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(&envelope.payload)
                .map_err(|_| ProtocolError::InvalidSignature)?,
        )
        .map_err(|_| ProtocolError::InvalidSignature)?;
        claims.validate(now_ms)?;
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
fn message(id: &str, payload: &str) -> Vec<u8> {
    format!("flare.plugin.call.v1\n{id}\n{payload}").into_bytes()
}
