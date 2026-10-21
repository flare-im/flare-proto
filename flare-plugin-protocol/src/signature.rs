//! Detached publisher signatures cover the exact manifest bytes. Plugin signing
//! keys are independent of Cell identity keys and may be removed on revocation.

use std::collections::BTreeMap;
use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::manifest::MAX_MANIFEST_BYTES;
use crate::{PluginManifest, ProtocolError, Result};

#[derive(Debug, Clone)]
pub struct VerifiedManifest {
    manifest: Arc<PluginManifest>,
    digest: String,
    key_id: String,
}

impl VerifiedManifest {
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
}

#[derive(Default)]
pub struct PublisherTrustStore {
    keys: BTreeMap<String, (String, VerifyingKey)>,
}

impl PublisherTrustStore {
    pub fn insert(&mut self, key_id: String, publisher_id: String, key: [u8; 32]) -> Result<()> {
        if key_id.is_empty() || publisher_id.is_empty() || self.keys.contains_key(&key_id) {
            return Err(ProtocolError::InvalidSignature);
        }
        let key = VerifyingKey::from_bytes(&key).map_err(|_| ProtocolError::InvalidSignature)?;
        self.keys.insert(key_id, (publisher_id, key));
        Ok(())
    }

    pub fn remove(&mut self, key_id: &str) {
        self.keys.remove(key_id);
    }

    pub fn verify(
        &self,
        bytes: &[u8],
        key_id: &str,
        signature_base64: &str,
    ) -> Result<VerifiedManifest> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ProtocolError::ManifestTooLarge(MAX_MANIFEST_BYTES));
        }
        let (publisher_id, key) = self
            .keys
            .get(key_id)
            .ok_or(ProtocolError::InvalidSignature)?;
        // Bound before decoding attacker-controlled base64.
        if signature_base64.len() > 128 {
            return Err(ProtocolError::InvalidSignature);
        }
        let signature_bytes = STANDARD
            .decode(signature_base64)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        let signature =
            Signature::from_slice(&signature_bytes).map_err(|_| ProtocolError::InvalidSignature)?;
        key.verify_strict(bytes, &signature)
            .map_err(|_| ProtocolError::InvalidSignature)?;
        let manifest = PluginManifest::from_json(bytes)?;
        if &manifest.publisher.publisher_id != publisher_id {
            return Err(ProtocolError::InvalidSignature);
        }
        Ok(VerifiedManifest {
            manifest: Arc::new(manifest),
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            key_id: key_id.to_owned(),
        })
    }
}
