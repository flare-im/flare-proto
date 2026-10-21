//! Deterministic metadata registry. Resolution pins immutable manifest digests;
//! no network, marketplace calls, process deployment or tenant state is involved.

use std::collections::{BTreeMap, BTreeSet};

use semver::{Version, VersionReq};

use crate::signature::VerifiedManifest;
use crate::{ProtocolError, Result};

const MAX_RESOLUTION_STEPS: usize = 4096;

#[derive(Default)]
pub struct PluginRegistry {
    versions: BTreeMap<String, BTreeMap<Version, VerifiedManifest>>,
    namespace_owners: BTreeMap<String, String>,
}

impl PluginRegistry {
    pub fn publish(&mut self, verified: VerifiedManifest) -> Result<()> {
        let manifest = verified.manifest();
        let version = Version::parse(&manifest.version).map_err(|_| ProtocolError::InvalidJson)?;
        if self
            .versions
            .get(&manifest.plugin_id)
            .and_then(|versions| versions.values().next())
            .is_some_and(|existing| {
                existing.manifest().publisher.publisher_id != manifest.publisher.publisher_id
            })
        {
            return Err(ProtocolError::PublisherConflict(manifest.plugin_id.clone()));
        }
        if let Some(existing) = self
            .versions
            .get(&manifest.plugin_id)
            .and_then(|versions| versions.get(&version))
        {
            return if existing.digest() == verified.digest() {
                Ok(())
            } else {
                Err(ProtocolError::VersionConflict(format!(
                    "{}@{}",
                    manifest.plugin_id, version
                )))
            };
        }
        for capability in &manifest.capabilities {
            // Prefix overlap is ownership overlap: `im.tools` cannot be claimed
            // by another plugin when `im.tools.search` is already registered.
            if self.namespace_owners.iter().any(|(namespace, owner)| {
                owner != &manifest.plugin_id
                    && (namespace == &capability.id
                        || namespace.starts_with(&format!("{}.", capability.id))
                        || capability.id.starts_with(&format!("{namespace}.")))
            }) {
                return Err(ProtocolError::NamespaceConflict(capability.id.clone()));
            }
        }
        let id = manifest.plugin_id.clone();
        for cap in &manifest.capabilities {
            self.namespace_owners.insert(cap.id.clone(), id.clone());
        }
        self.versions
            .entry(id)
            .or_default()
            .insert(version, verified);
        Ok(())
    }

    /// Resolve a complete dependency closure with backtracking. Picking the
    /// newest version greedily would reject valid diamond dependency graphs.
    pub fn resolve(
        &self,
        plugin_id: &str,
        version_range: &str,
        host_version: &str,
    ) -> Result<Vec<VerifiedManifest>> {
        let req = VersionReq::parse(version_range)
            .map_err(|_| ProtocolError::UnresolvedDependency(plugin_id.to_owned()))?;
        let host = Version::parse(host_version)
            .map_err(|_| ProtocolError::UnresolvedDependency("host version".to_owned()))?;
        let mut constraints = BTreeMap::new();
        constraints.insert(plugin_id.to_owned(), vec![req]);
        let mut cycle = None;
        let selected = self
            .solve(BTreeMap::new(), constraints, &host, &mut 0, &mut cycle)
            .ok_or_else(|| {
                cycle
                    .map(ProtocolError::DependencyCycle)
                    .unwrap_or_else(|| ProtocolError::UnresolvedDependency(plugin_id.to_owned()))
            })?;
        let mut visited = BTreeSet::new();
        let mut active = BTreeSet::new();
        let mut ordered = Vec::new();
        visit(
            plugin_id,
            &selected,
            &mut visited,
            &mut active,
            &mut ordered,
        )?;
        Ok(ordered)
    }

    fn solve(
        &self,
        selected: BTreeMap<String, VerifiedManifest>,
        constraints: BTreeMap<String, Vec<VersionReq>>,
        host: &Version,
        steps: &mut usize,
        cycle: &mut Option<String>,
    ) -> Option<BTreeMap<String, VerifiedManifest>> {
        *steps += 1;
        if *steps > MAX_RESOLUTION_STEPS || selected.len() > 128 {
            return None;
        }
        for (id, verified) in &selected {
            let version = Version::parse(&verified.manifest().version).ok()?;
            if !constraints.get(id)?.iter().all(|req| req.matches(&version)) {
                return None;
            }
        }
        let Some((id, requirements)) = constraints
            .iter()
            .find(|(id, _)| !selected.contains_key(*id))
        else {
            let mut visited = BTreeSet::new();
            for id in selected.keys() {
                if let Err(ProtocolError::DependencyCycle(id)) = visit(
                    id,
                    &selected,
                    &mut visited,
                    &mut BTreeSet::new(),
                    &mut Vec::new(),
                ) {
                    *cycle = Some(id);
                    return None;
                }
            }
            return Some(selected);
        };
        let versions = self.versions.get(id)?;
        for (version, verified) in versions.iter().rev() {
            if !requirements.iter().all(|req| req.matches(version)) {
                continue;
            }
            let manifest = verified.manifest();
            if manifest.host_version_range.as_ref().is_some_and(|range| {
                VersionReq::parse(range).map_or(true, |req| !req.matches(host))
            }) {
                continue;
            }
            let mut next_selected = selected.clone();
            next_selected.insert(id.clone(), verified.clone());
            let mut next_constraints = constraints.clone();
            for dep in &manifest.dependencies {
                if dep.optional && !self.versions.contains_key(&dep.plugin_id) {
                    continue;
                }
                next_constraints
                    .entry(dep.plugin_id.clone())
                    .or_default()
                    .push(VersionReq::parse(&dep.version_range).ok()?);
            }
            if let Some(solution) = self.solve(next_selected, next_constraints, host, steps, cycle)
            {
                return Some(solution);
            }
        }
        None
    }
}

fn visit(
    id: &str,
    selected: &BTreeMap<String, VerifiedManifest>,
    visited: &mut BTreeSet<String>,
    active: &mut BTreeSet<String>,
    ordered: &mut Vec<VerifiedManifest>,
) -> Result<()> {
    if visited.contains(id) {
        return Ok(());
    }
    if !active.insert(id.to_owned()) {
        return Err(ProtocolError::DependencyCycle(id.to_owned()));
    }
    let verified = selected
        .get(id)
        .ok_or_else(|| ProtocolError::UnresolvedDependency(id.to_owned()))?;
    for dep in &verified.manifest().dependencies {
        if selected.contains_key(&dep.plugin_id) {
            visit(&dep.plugin_id, selected, visited, active, ordered)?;
        }
    }
    active.remove(id);
    visited.insert(id.to_owned());
    ordered.push(verified.clone());
    Ok(())
}
