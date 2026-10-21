//! Structural endpoint declaration validation shared by Core and plugin SDKs.
//! A declaration is not evidence of publisher trust or tenant approval.

use std::collections::BTreeSet;

use crate::manifest::PLUGIN_API_VERSION;

pub fn validate_declaration(
    plugin_version: &str,
    api_version: &str,
    manifest_sha256: &str,
    declared_operations: &[String],
    capability_id: &str,
) -> std::result::Result<(), &'static str> {
    if plugin_version.len() > 64 || semver::Version::parse(plugin_version).is_err() {
        return Err("plugin_version must be a semantic version");
    }
    if api_version != PLUGIN_API_VERSION {
        return Err("api_version must be 1.0");
    }
    if manifest_sha256.len() != 64
        || !manifest_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("manifest_sha256 must be 64 lowercase hexadecimal characters");
    }
    if declared_operations.is_empty() || declared_operations.len() > 256 {
        return Err("declared_operations must contain 1 to 256 operations");
    }
    let mut seen = BTreeSet::new();
    for op in declared_operations {
        if op.is_empty()
            || op.len() > 128
            || !op
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
            || !seen.insert(op)
        {
            return Err("declared_operations must contain unique bounded identifiers");
        }
    }
    if !declared_operations.iter().any(|op| op == capability_id) {
        return Err("capability_id must be present in declared_operations");
    }
    Ok(())
}
