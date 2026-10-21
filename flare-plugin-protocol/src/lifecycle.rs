use serde::{Deserialize, Serialize};

use crate::{ProtocolError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Staged,
    Validated,
    Starting,
    Running,
    Draining,
    Stopped,
    Failed,
}

/// Pure lifecycle contract. A runtime adapter persists the revision and performs
/// external effects with fencing; tenant enable/disable is a separate policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginLifecycle {
    state: RuntimeState,
    revision: u64,
}

impl Default for PluginLifecycle {
    fn default() -> Self {
        Self {
            state: RuntimeState::Staged,
            revision: 0,
        }
    }
}

impl PluginLifecycle {
    pub fn state(&self) -> RuntimeState {
        self.state
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn transition(&mut self, expected_revision: u64, next: RuntimeState) -> Result<u64> {
        if self.revision != expected_revision {
            return Err(ProtocolError::StaleRevision);
        }
        use RuntimeState::*;
        let allowed = matches!(
            (self.state, next),
            (Staged, Validated | Failed)
                | (Validated | Stopped, Starting)
                | (Starting, Running | Failed | Stopped)
                | (Running, Draining | Failed)
                | (Draining, Stopped | Failed)
                | (Failed, Validated | Stopped)
        );
        if !allowed {
            return Err(ProtocolError::InvalidTransition);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(ProtocolError::StaleRevision)?;
        self.state = next;
        self.revision = revision;
        Ok(revision)
    }
}
