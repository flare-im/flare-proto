//! Request-bound registration delegation. These credentials never authorize a
//! user call, resource callback, runtime configuration or tenant installation.
use crate::{ProtocolError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const REGISTRATION_HEADER: &str = "x-flare-plugin-registration";
pub const REGISTRATION_TOKEN_USE: &str = "plugin_registration";
pub const REGISTRATION_ISSUER: &str = "flare-platform";
pub const REGISTRATION_TTL_SECS: u64 = 60;
pub const WORKLOAD_AUDIENCE: &str = "flare-plugin-broker";
pub const BROKER_PATH: &str = "/api/v1/internal/plugins/registration-tokens";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndpointDeclaration {
    pub capability_id: String,
    pub grpc_authority: String,
    pub plugin_version: String,
    pub api_version: String,
    pub manifest_sha256: String,
    pub declared_operations: Vec<String>,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "declaration",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RegistrationOperation {
    Register(EndpointDeclaration),
    Deregister,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationRequest {
    pub plugin_id: String,
    pub tenant_id: String,
    pub request_id: String,
    pub action: RegistrationOperation,
}
impl RegistrationRequest {
    pub fn validate(&self) -> Result<()> {
        if !identifier(&self.plugin_id, 128)
            || !identifier(&self.request_id, 192)
            || self.tenant_id.is_empty()
            || self.tenant_id.len() > 32
            || !self
                .tenant_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Err(ProtocolError::Denied("invalid registration identity"));
        }
        if let RegistrationOperation::Register(d) = &self.action {
            crate::registration::validate_declaration(
                &d.plugin_version,
                &d.api_version,
                &d.manifest_sha256,
                &d.declared_operations,
                &d.capability_id,
            )
            .map_err(ProtocolError::Denied)?;
            if !crate::runtime::valid_grpc_authority(&d.grpc_authority)
                || d.labels.len() > 16
                || d.labels.iter().any(|(k, v)| {
                    !identifier(k, 64) || v.len() > 256 || v.chars().any(char::is_control)
                })
            {
                return Err(ProtocolError::Denied("invalid registration endpoint"));
            }
        }
        Ok(())
    }
    /// Sorted label keys and exact operation ordering are part of the wire
    /// contract. A nonce reused with a different body has a different audience.
    pub fn audience(&self) -> Result<String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| ProtocolError::InvalidJson)?;
        Ok(format!(
            "flare-plugin-registration:{:x}",
            Sha256::digest(bytes)
        ))
    }
}
fn identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationExchange {
    pub deployment_id: String,
    pub request: RegistrationRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationCredential {
    pub token: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> RegistrationRequest {
        RegistrationRequest {
            plugin_id: "vendor.plugin".into(),
            tenant_id: "app-a".into(),
            request_id: "request-1".into(),
            action: RegistrationOperation::Register(EndpointDeclaration {
                capability_id: "vendor.read".into(),
                grpc_authority: "https://plugin:7900".into(),
                plugin_version: "1.0.0".into(),
                api_version: "1.0".into(),
                manifest_sha256: "a".repeat(64),
                declared_operations: vec!["vendor.read".into()],
                labels: BTreeMap::new(),
            }),
        }
    }
    #[test]
    fn delegation_is_bound_to_every_request_field_and_operation() {
        let original = request();
        let expected = original.audience().unwrap();
        for change in 0..9 {
            let mut r = original.clone();
            match change {
                0 => r.plugin_id = "other.plugin".into(),
                1 => r.tenant_id = "app-b".into(),
                2 => r.request_id = "request-2".into(),
                3 => r.action = RegistrationOperation::Deregister,
                _ => {
                    let RegistrationOperation::Register(d) = &mut r.action else {
                        unreachable!()
                    };
                    match change {
                        4 => d.grpc_authority = "https://other:7900".into(),
                        5 => d.plugin_version = "2.0.0".into(),
                        6 => d.manifest_sha256 = "b".repeat(64),
                        7 => d.declared_operations.push("vendor.write".into()),
                        _ => {
                            d.labels.insert("health_protocol".into(), "other".into());
                        }
                    }
                }
            }
            assert_ne!(r.audience().unwrap(), expected);
        }
    }
    #[test]
    fn rejects_unknown_fields_wildcards_oversize_and_invalid_addresses() {
        let mut r = request();
        r.tenant_id = "*".into();
        assert!(r.validate().is_err());
        r = request();
        r.request_id = "x".repeat(193);
        assert!(r.validate().is_err());
        r = request();
        let RegistrationOperation::Register(d) = &mut r.action else {
            unreachable!()
        };
        d.grpc_authority = "http://user:secret@plugin:7900".into();
        assert!(r.validate().is_err());
        let mut v = serde_json::to_value(request()).unwrap();
        v["actorId"] = "forged".into();
        assert!(serde_json::from_value::<RegistrationRequest>(v).is_err());
    }
}
